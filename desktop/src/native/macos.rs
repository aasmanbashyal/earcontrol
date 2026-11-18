//! macOS media capture and input delivery. No Bluetooth pairing or audio playback.
//!
//! Headphone and keyboard media buttons reach EarControl through two routes,
//! mirroring the Windows build:
//! - Now Playing: while capture runs, EarControl registers MPRemoteCommandCenter
//!   handlers and reports itself as playing, so macOS can route remote commands
//!   (AirPods and other Bluetooth headphones) to it.
//! - Keyboard media keys: an optional Quartz event tap sees the system media
//!   keys and consumes mapped ones. It needs Accessibility access.
//!
//! Shortcuts and text are posted as Quartz keyboard events, which also need
//! Accessibility access.
use crate::model::{DeviceInfo, TargetWindow};
use block2::RcBlock;
use dispatch2::DispatchQueue;
use objc2::{
    encode::{Encoding, RefEncode},
    msg_send,
    rc::{autoreleasepool, Retained},
    runtime::AnyObject,
    MainThreadMarker,
};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use objc2_media_player::{
    MPMediaItemPropertyArtist, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
    MPNowPlayingPlaybackState, MPRemoteCommand, MPRemoteCommandCenter, MPRemoteCommandEvent,
    MPRemoteCommandHandlerStatus,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr::{null, null_mut, NonNull},
    sync::{
        atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
    time::Instant,
};

const NOW_PLAYING_SOURCE: &str = "macOS Now Playing";
const KEYBOARD_SOURCE: &str = "Keyboard media key";
// Marks events EarControl posts, like the Windows dwExtraInfo tag.
const INJECTED_TAG: i64 = 0x4541_5243;

type CommandCallback = Arc<dyn Fn(String, String, u64, Instant) + Send + Sync>;
type ConsumeCallback = Arc<dyn Fn(&str) -> bool + Send + Sync>;
type HealthCallback = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureRoute {
    #[default]
    Checking,
    EarControl,
    OtherApp,
    None,
    Unavailable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CaptureHealth {
    pub route: CaptureRoute,
    pub keyboard_fallback: bool,
    pub input_thread_alive: bool,
    // Reports accepted by this capture generation, before duplicate filtering.
    pub media_commands: u64,
    pub keyboard_commands: u64,
}

#[derive(Default)]
struct ReceivedCounts {
    generation: u64,
    media: u64,
    keyboard: u64,
}

#[derive(Default)]
struct SessionState {
    listening: AtomicBool,
    keyboard_fallback: AtomicBool,
    generation: AtomicU64,
    counts: Mutex<ReceivedCounts>,
    health_changed: OnceLock<HealthCallback>,
}

impl SessionState {
    fn invalidate(&self) {
        self.listening.store(false, Ordering::SeqCst);
        self.keyboard_fallback.store(false, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn notify_health(&self) {
        if let Some(changed) = self.health_changed.get() {
            changed();
        }
    }

    fn record_report(&self, event: &CapturedCommand) -> bool {
        let mut counts = self.counts.lock().unwrap();
        if !self.listening.load(Ordering::SeqCst)
            || event.generation != self.generation.load(Ordering::SeqCst)
        {
            return false;
        }
        if counts.generation != event.generation {
            *counts = ReceivedCounts {
                generation: event.generation,
                ..Default::default()
            };
        }
        match event.source {
            NOW_PLAYING_SOURCE => counts.media = counts.media.saturating_add(1),
            KEYBOARD_SOURCE => counts.keyboard = counts.keyboard.saturating_add(1),
            _ => {}
        }
        true
    }

    fn health(&self, input_thread_alive: bool) -> CaptureHealth {
        let counts = self.counts.lock().unwrap();
        let current = counts.generation == self.generation.load(Ordering::SeqCst);
        CaptureHealth {
            // macOS has no public API that names the Now Playing app.
            route: CaptureRoute::Unavailable,
            keyboard_fallback: self.keyboard_fallback.load(Ordering::SeqCst),
            input_thread_alive,
            media_commands: if current { counts.media } else { 0 },
            keyboard_commands: if current { counts.keyboard } else { 0 },
        }
    }
}

struct CapturedCommand {
    generation: u64,
    trigger: &'static str,
    source: &'static str,
    captured_at: Instant,
}

fn enqueue(
    session: &SessionState,
    events: &SyncSender<CapturedCommand>,
    trigger: &'static str,
    source: &'static str,
) -> bool {
    let generation = session.generation.load(Ordering::SeqCst);
    if !session.listening.load(Ordering::SeqCst) {
        return false;
    }
    events
        .try_send(CapturedCommand {
            generation,
            trigger,
            source,
            captured_at: Instant::now(),
        })
        .is_ok()
}

pub struct NativeInput {
    session: Arc<SessionState>,
    events: SyncSender<CapturedCommand>,
    should_consume: ConsumeCallback,
    tap: Mutex<Option<EventTap>>,
    dispatcher: Option<JoinHandle<()>>,
}

impl NativeInput {
    pub fn new(
        window: isize,
        on_command: CommandCallback,
        should_consume: ConsumeCallback,
        health_changed: HealthCallback,
    ) -> Result<Self, String> {
        if window == 0 {
            return Err("The EarControl window has not been created yet.".into());
        }
        let session = Arc::new(SessionState::default());
        let _ = session.health_changed.set(health_changed);
        let (events, event_receiver) = mpsc::sync_channel::<CapturedCommand>(64);
        let callback_session = Arc::clone(&session);
        // Run user callbacks away from the main thread and the event tap.
        let dispatcher = thread::Builder::new()
            .name("earcontrol-media-events".into())
            .spawn(move || {
                while let Ok(event) = event_receiver.recv() {
                    if callback_session.record_report(&event) {
                        let _ = catch_unwind(AssertUnwindSafe(|| {
                            on_command(
                                event.trigger.into(),
                                event.source.into(),
                                event.generation,
                                event.captured_at,
                            );
                        }));
                        callback_session.notify_health();
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            session,
            events,
            should_consume,
            tap: Mutex::new(None),
            dispatcher: Some(dispatcher),
        })
    }

    pub fn start(&self, keyboard_fallback: bool) -> Result<(), String> {
        self.session.invalidate();
        if keyboard_fallback {
            let mut tap = self.tap.lock().unwrap();
            if tap.as_ref().is_none_or(|tap| !tap.is_alive()) {
                *tap = Some(EventTap::start(
                    Arc::clone(&self.session),
                    self.events.clone(),
                    Arc::clone(&self.should_consume),
                )?);
            } else if let Some(tap) = tap.as_ref() {
                tap.enable();
            }
        }
        let session = Arc::clone(&self.session);
        let events = self.events.clone();
        on_main(move || enable_remote_commands(session, events));
        self.session
            .keyboard_fallback
            .store(keyboard_fallback, Ordering::SeqCst);
        self.session.listening.store(true, Ordering::SeqCst);
        self.session.notify_health();
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.session.invalidate();
        on_main(disable_remote_commands);
        self.session.notify_health();
        Ok(())
    }

    pub fn generation(&self) -> u64 {
        self.session.generation.load(Ordering::SeqCst)
    }

    pub fn capture_health(&self) -> CaptureHealth {
        let tap_alive = self
            .tap
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(EventTap::is_alive);
        self.session.health(
            tap_alive
                && self
                    .dispatcher
                    .as_ref()
                    .is_some_and(|thread| !thread.is_finished()),
        )
    }
}

impl Drop for NativeInput {
    fn drop(&mut self) {
        self.session.invalidate();
        on_main(disable_remote_commands);
        self.tap.lock().unwrap().take();
        // Dropping the last sender ends the dispatcher. A callback may own the
        // final AppState reference, so never join from the dispatcher itself.
        let (closed, _) = mpsc::sync_channel(1);
        drop(std::mem::replace(&mut self.events, closed));
        if let Some(thread) = self.dispatcher.take() {
            if thread.thread().id() != thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}

/// MediaPlayer objects belong to the main thread. Work queued from another
/// thread runs asynchronously so a waiting UI thread can never deadlock.
fn on_main(work: impl FnOnce() + Send + 'static) {
    if MainThreadMarker::new().is_some() {
        work();
    } else {
        DispatchQueue::main().exec_async(work);
    }
}

thread_local! {
    // Main thread only: each registered command and the target it returned.
    static REMOTE_TARGETS: RefCell<Vec<(Retained<MPRemoteCommand>, Retained<AnyObject>)>> =
        const { RefCell::new(Vec::new()) };
}

fn enable_remote_commands(session: Arc<SessionState>, events: SyncSender<CapturedCommand>) {
    disable_remote_commands();
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        let commands = [
            (center.togglePlayPauseCommand(), "play_pause"),
            (center.playCommand(), "play_pause"),
            (center.pauseCommand(), "play_pause"),
            (center.nextTrackCommand(), "next"),
            (center.previousTrackCommand(), "previous"),
            (center.stopCommand(), "stop"),
        ];
        let mut registered = Vec::with_capacity(commands.len());
        for (command, trigger) in commands {
            let session = Arc::clone(&session);
            let events = events.clone();
            let handler = RcBlock::new(move |_event: NonNull<MPRemoteCommandEvent>| {
                enqueue(&session, &events, trigger, NOW_PLAYING_SOURCE);
                MPRemoteCommandHandlerStatus::Success
            });
            command.setEnabled(true);
            let target = command.addTargetWithHandler(&handler);
            registered.push((command, target));
        }
        REMOTE_TARGETS.with(|targets| *targets.borrow_mut() = registered);

        let title = NSString::from_str("EarControl");
        let artist = NSString::from_str("Headphone remote");
        let info = NSDictionary::<NSString, AnyObject>::from_slices(
            &[MPMediaItemPropertyTitle, MPMediaItemPropertyArtist],
            &[title.as_ref(), artist.as_ref()],
        );
        let now_playing = MPNowPlayingInfoCenter::defaultCenter();
        now_playing.setNowPlayingInfo(Some(&info));
        // On macOS this state, not an audio session, makes the app eligible
        // to receive remote commands. EarControl never plays audio.
        now_playing.setPlaybackState(MPNowPlayingPlaybackState::Playing);
    }
}

fn disable_remote_commands() {
    let registered = REMOTE_TARGETS.with(|targets| std::mem::take(&mut *targets.borrow_mut()));
    unsafe {
        for (command, target) in registered {
            command.removeTarget(Some(&target));
            command.setEnabled(false);
        }
        let now_playing = MPNowPlayingInfoCenter::defaultCenter();
        now_playing.setPlaybackState(MPNowPlayingPlaybackState::Stopped);
        now_playing.setNowPlayingInfo(None);
    }
}

// ---------------------------------------------------------------------------
// Quartz event services

#[repr(C)]
struct CGEvent {
    _private: [u8; 0],
}

unsafe impl RefEncode for CGEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}

type CGEventRef = *mut CGEvent;
type CFTypeRef = *const c_void;
type TapCallback = unsafe extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

const SESSION_EVENT_TAP: u32 = 1;
const HID_EVENT_TAP: u32 = 0;
const HEAD_INSERT: u32 = 0;
const TAP_DEFAULT: u32 = 0;
const SYSTEM_DEFINED: u32 = 14;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
const SOURCE_COMBINED_SESSION: i32 = 0;
const SOURCE_HID_SYSTEM: i32 = 1;
const SOURCE_USER_DATA_FIELD: u32 = 42;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;
const WINDOW_LIST_ALL: u32 = 0;
const WINDOW_LIST_ON_SCREEN: u32 = 1;
const WINDOW_LIST_EXCLUDE_DESKTOP: u32 = 16;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventTapIsEnabled(tap: *mut c_void) -> bool;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventSourceCreate(state: i32) -> *mut c_void;
    fn CGEventSourceFlagsState(state: i32) -> u64;
    fn CGEventCreateKeyboardEvent(source: *mut c_void, key: u16, down: bool) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventKeyboardSetUnicodeString(event: CGEventRef, length: usize, text: *const u16);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFTypeRef;
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: CFTypeRef,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFMachPortInvalidate(port: *mut c_void);
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: CFTypeRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(run_loop: *mut c_void);
    fn CFRetain(object: CFTypeRef) -> CFTypeRef;
    fn CFRelease(object: CFTypeRef);
    static kCFRunLoopCommonModes: CFTypeRef;
}

const ACCESSIBILITY_HELP: &str =
    "Allow EarControl in System Settings › Privacy & Security › Accessibility, then retry.";

/// Checks Accessibility access, asking macOS to show its prompt when missing.
fn accessibility_trusted(prompt: bool) -> bool {
    if !prompt {
        return unsafe { AXIsProcessTrusted() };
    }
    unsafe {
        let key = &*(kAXTrustedCheckOptionPrompt as *const NSString);
        let yes = NSNumber::new_bool(true);
        let options = NSDictionary::<NSString, AnyObject>::from_slices(&[key], &[yes.as_ref()]);
        AXIsProcessTrustedWithOptions(Retained::as_ptr(&options).cast())
    }
}

struct CFRef(*mut c_void);
// CoreFoundation run loops and mach ports may be referenced from any thread.
unsafe impl Send for CFRef {}
unsafe impl Sync for CFRef {}
impl Drop for CFRef {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

#[derive(Default)]
struct PressedKeys(HashMap<i64, (u64, bool)>);

impl PressedKeys {
    fn repeat(&self, key: i64, generation: u64, listening: bool) -> Option<bool> {
        // A consumed key stays consumed until its key-up, even after Stop.
        self.0
            .get(&key)
            .filter(|(seen, consumed)| *seen == generation || (!listening && *consumed))
            .map(|(_, consumed)| *consumed)
    }
    fn down(&mut self, key: i64, generation: u64, consumed: bool) {
        self.0.insert(key, (generation, consumed));
    }
    fn up(&mut self, key: i64) -> bool {
        self.0.remove(&key).is_some_and(|(_, consumed)| consumed)
    }
}

struct TapState {
    session: Arc<SessionState>,
    events: SyncSender<CapturedCommand>,
    should_consume: ConsumeCallback,
    pressed: Mutex<PressedKeys>,
    port: AtomicPtr<c_void>,
}

struct EventTap {
    run_loop: Arc<CFRef>,
    port: Arc<CFRef>,
    thread: Option<JoinHandle<()>>,
}

impl EventTap {
    fn start(
        session: Arc<SessionState>,
        events: SyncSender<CapturedCommand>,
        should_consume: ConsumeCallback,
    ) -> Result<Self, String> {
        if !accessibility_trusted(true) {
            return Err(format!(
                "Keyboard media keys need Accessibility access. {ACCESSIBILITY_HELP}"
            ));
        }
        let (ready, receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("earcontrol-media-keys".into())
            .spawn(move || {
                let state = Box::into_raw(Box::new(TapState {
                    session,
                    events,
                    should_consume,
                    pressed: Mutex::default(),
                    port: AtomicPtr::new(null_mut()),
                }));
                unsafe {
                    let port = CGEventTapCreate(
                        SESSION_EVENT_TAP,
                        HEAD_INSERT,
                        TAP_DEFAULT,
                        1 << SYSTEM_DEFINED,
                        media_key_callback,
                        state.cast(),
                    );
                    if port.is_null() {
                        drop(Box::from_raw(state));
                        let _ = ready.send(Err(format!(
                            "Could not watch keyboard media keys. {ACCESSIBILITY_HELP}"
                        )));
                        return;
                    }
                    (*state).port.store(port, Ordering::SeqCst);
                    let source = CFMachPortCreateRunLoopSource(null(), port, 0);
                    let run_loop = CFRunLoopGetCurrent();
                    CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
                    CFRetain(run_loop);
                    CFRetain(port);
                    let _ = ready.send(Ok((CFRef(run_loop), CFRef(port))));
                    CFRunLoopRun();
                    CFMachPortInvalidate(port);
                    CFRelease(source);
                    CFRelease(port);
                    drop(Box::from_raw(state));
                }
            })
            .map_err(|error| error.to_string())?;
        match receiver.recv() {
            Ok(Ok((run_loop, port))) => Ok(Self {
                run_loop: Arc::new(run_loop),
                port: Arc::new(port),
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => Err("The media-key watcher stopped unexpectedly.".into()),
        }
    }

    fn enable(&self) {
        unsafe { CGEventTapEnable(self.port.0, true) };
    }

    fn is_alive(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
            && unsafe { CGEventTapIsEnabled(self.port.0) }
    }
}

impl Drop for EventTap {
    fn drop(&mut self) {
        unsafe { CFRunLoopStop(self.run_loop.0) };
        if let Some(thread) = self.thread.take() {
            if thread.thread().id() != thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}

/// NX_KEYTYPE values from IOKit's ev_keymap.h.
fn trigger_for_media_key(key: i64) -> Option<&'static str> {
    match key {
        0 => Some("volume_up"),
        1 => Some("volume_down"),
        7 => Some("mute"),
        16 => Some("play_pause"),
        17 | 19 => Some("next"),
        18 | 20 => Some("previous"),
        _ => None,
    }
}

/// Reads the media key from an NX_SYSDEFINED event: (key, pressed, repeat).
unsafe fn media_key(event: CGEventRef) -> Option<(i64, bool, bool)> {
    autoreleasepool(|_| {
        let class = objc2::runtime::AnyClass::get(c"NSEvent")?;
        let ns_event: Option<Retained<AnyObject>> =
            unsafe { msg_send![class, eventWithCGEvent: event] };
        let ns_event = ns_event?;
        let subtype: i16 = unsafe { msg_send![&*ns_event, subtype] };
        if subtype != 8 {
            return None;
        }
        let data: isize = unsafe { msg_send![&*ns_event, data1] };
        let data = data as i64;
        let key = (data & 0xFFFF_0000) >> 16;
        let state = (data & 0xFF00) >> 8;
        Some((key, state == 0x0A, data & 1 == 1))
    })
}

unsafe extern "C" fn media_key_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: CGEventRef,
    info: *mut c_void,
) -> CGEventRef {
    let state = unsafe { &*(info as *const TapState) };
    if matches!(
        event_type,
        TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT
    ) {
        // macOS turns off a slow tap. Re-enable it instead of losing the fallback.
        let port = state.port.load(Ordering::SeqCst);
        if !port.is_null() {
            unsafe { CGEventTapEnable(port, true) };
        }
        return event;
    }
    if event_type != SYSTEM_DEFINED
        || unsafe { CGEventGetIntegerValueField(event, SOURCE_USER_DATA_FIELD) } == INJECTED_TAG
    {
        return event;
    }
    let consumed = catch_unwind(AssertUnwindSafe(|| {
        let Some((key, pressed, _repeat)) = (unsafe { media_key(event) }) else {
            return false;
        };
        let Some(trigger) = trigger_for_media_key(key) else {
            return false;
        };
        let mut keys = state.pressed.lock().unwrap();
        if !pressed {
            return keys.up(key);
        }
        let generation = state.session.generation.load(Ordering::SeqCst);
        let listening = state.session.listening.load(Ordering::SeqCst);
        if let Some(consumed) = keys.repeat(key, generation, listening) {
            return consumed;
        }
        let mut consume = false;
        if listening && state.session.keyboard_fallback.load(Ordering::SeqCst) {
            consume = (state.should_consume)(trigger);
            consume &= enqueue(&state.session, &state.events, trigger, KEYBOARD_SOURCE);
        }
        keys.down(key, generation, consume);
        consume
    }))
    .unwrap_or(false);
    if consumed {
        null_mut()
    } else {
        event
    }
}

// ---------------------------------------------------------------------------
// Shortcuts and text

pub fn win_key_held() -> bool {
    unsafe { CGEventSourceFlagsState(SOURCE_COMBINED_SESSION) & FLAG_COMMAND != 0 }
}

pub fn modifiers_held() -> bool {
    let flags = unsafe { CGEventSourceFlagsState(SOURCE_COMBINED_SESSION) };
    flags & (FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND) != 0
}

/// Maps the Windows virtual-key codes stored in profiles to macOS key codes
/// (kVK_* from HIToolbox Events.h) and, for modifiers, their event flag.
fn mac_key(key: u16) -> Option<(u16, u64)> {
    let modifier = match key {
        0x10 | 0xA0 | 0xA1 => Some((0x38, FLAG_SHIFT)),
        0x11 | 0xA2 | 0xA3 => Some((0x3B, FLAG_CONTROL)),
        0x12 | 0xA4 | 0xA5 => Some((0x3A, FLAG_OPTION)),
        0x5B | 0x5C => Some((0x37, FLAG_COMMAND)),
        _ => None,
    };
    if modifier.is_some() {
        return modifier;
    }
    const LETTERS: [u16; 26] = [
        0x00, 0x0B, 0x08, 0x02, 0x0E, 0x03, 0x05, 0x04, 0x22, 0x26, 0x28, 0x25, 0x2E, 0x2D, 0x1F,
        0x23, 0x0C, 0x0F, 0x01, 0x11, 0x20, 0x09, 0x0D, 0x07, 0x10, 0x06,
    ];
    const DIGITS: [u16; 10] = [0x1D, 0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19];
    const FUNCTION: [u16; 20] = [
        0x7A, 0x78, 0x63, 0x76, 0x60, 0x61, 0x62, 0x64, 0x65, 0x6D, 0x67, 0x6F, 0x69, 0x6B, 0x71,
        0x6A, 0x40, 0x4F, 0x50, 0x5A,
    ];
    let code = match key {
        0x41..=0x5A => LETTERS[(key - 0x41) as usize],
        0x30..=0x39 => DIGITS[(key - 0x30) as usize],
        0x70..=0x83 => FUNCTION[(key - 0x70) as usize],
        0x08 => 0x33, // Backspace
        0x09 => 0x30, // Tab
        0x0D => 0x24, // Return
        0x1B => 0x35, // Escape
        0x20 => 0x31, // Space
        0x21 => 0x74, // Page Up
        0x22 => 0x79, // Page Down
        0x23 => 0x77, // End
        0x24 => 0x73, // Home
        0x25 => 0x7B, // Left
        0x26 => 0x7E, // Up
        0x27 => 0x7C, // Right
        0x28 => 0x7D, // Down
        0x2D => 0x72, // Insert (Help)
        0x2E => 0x75, // Forward delete
        _ => return None,
    };
    Some((code, 0))
}

fn post_key(source: *mut c_void, code: u16, down: bool, flags: u64) -> Result<(), String> {
    unsafe {
        let event = CGEventCreateKeyboardEvent(source, code, down);
        if event.is_null() {
            return Err("macOS could not create the shortcut event.".into());
        }
        CGEventSetFlags(event, flags);
        CGEventSetIntegerValueField(event, SOURCE_USER_DATA_FIELD, INJECTED_TAG);
        CGEventPost(HID_EVENT_TAP, event);
        CFRelease(event as CFTypeRef);
    }
    Ok(())
}

fn input_source() -> Result<CFRef, String> {
    if !accessibility_trusted(true) {
        return Err(format!(
            "Sending shortcuts needs Accessibility access. {ACCESSIBILITY_HELP}"
        ));
    }
    let source = unsafe { CGEventSourceCreate(SOURCE_HID_SYSTEM) };
    if source.is_null() {
        return Err("macOS could not create a keyboard event source.".into());
    }
    Ok(CFRef(source))
}

pub fn send_keys(keys: &[u16]) -> Result<(), String> {
    if keys.is_empty() {
        return Ok(());
    }
    let mapped = keys
        .iter()
        .map(|key| mac_key(*key))
        .collect::<Option<Vec<_>>>()
        .filter(|keys| keys.len() <= 5)
        .ok_or("The shortcut contains keys macOS cannot send. F21 to F24 are unavailable.")?;
    if modifiers_held() {
        return Err("Release Control, Option, Shift, and Command before running a macro.".into());
    }
    let source = input_source()?;
    // Press in order with cumulative modifier flags, then release in reverse.
    let mut flags = 0;
    let mut pressed = Vec::with_capacity(mapped.len());
    let mut result = Ok(());
    for (code, flag) in &mapped {
        flags |= flag;
        result = post_key(source.0, *code, true, flags);
        if result.is_err() {
            break;
        }
        pressed.push((*code, *flag));
    }
    for (code, flag) in pressed.into_iter().rev() {
        let _ = post_key(source.0, code, false, flags);
        flags &= !flag;
    }
    result
}

/// Types Unicode text directly. It never reads or replaces the clipboard.
pub fn send_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    if text.chars().count() > 2000 {
        return Err("A text step can contain at most 2,000 characters.".into());
    }
    if modifiers_held() {
        return Err("Release Control, Option, Shift, and Command before running a macro.".into());
    }
    let source = input_source()?;
    let units: Vec<u16> = text.encode_utf16().collect();
    // Quartz accepts at most 20 UTF-16 units per event. Keep surrogate pairs together.
    let mut start = 0;
    while start < units.len() {
        let mut end = (start + 20).min(units.len());
        if end < units.len() && (0xD800..0xDC00).contains(&units[end - 1]) {
            end -= 1;
        }
        for down in [true, false] {
            unsafe {
                let event = CGEventCreateKeyboardEvent(source.0, 0, down);
                if event.is_null() {
                    return Err("macOS could not finish typing the macro text.".into());
                }
                CGEventKeyboardSetUnicodeString(event, end - start, units[start..].as_ptr());
                CGEventSetIntegerValueField(event, SOURCE_USER_DATA_FIELD, INJECTED_TAG);
                CGEventPost(HID_EVENT_TAP, event);
                CFRelease(event as CFTypeRef);
            }
        }
        start = end;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Windows

pub(crate) struct WindowEntry {
    pub number: u32,
    pub process_id: u32,
    pub layer: i64,
    pub owner: String,
    pub name: String,
}

/// Quartz window list, front to back. Names of other apps' windows need
/// Screen Recording access; without it only the owning app's name is known.
pub(crate) fn window_entries(on_screen_only: bool) -> Vec<WindowEntry> {
    let option = if on_screen_only {
        WINDOW_LIST_ON_SCREEN | WINDOW_LIST_EXCLUDE_DESKTOP
    } else {
        WINDOW_LIST_ALL
    };
    autoreleasepool(|_| {
        let list = unsafe { CGWindowListCopyWindowInfo(option, 0) };
        if list.is_null() {
            return Vec::new();
        }
        // CFArrayRef and CFDictionaryRef are toll-free bridged to Foundation.
        let Some(list) = (unsafe {
            Retained::from_raw(list as *mut NSArray<NSDictionary<NSString, AnyObject>>)
        }) else {
            return Vec::new();
        };
        let key = |name: &str| NSString::from_str(name);
        let (number_key, pid_key, layer_key, owner_key, name_key) = (
            key("kCGWindowNumber"),
            key("kCGWindowOwnerPID"),
            key("kCGWindowLayer"),
            key("kCGWindowOwnerName"),
            key("kCGWindowName"),
        );
        let number = |row: &NSDictionary<NSString, AnyObject>, key: &NSString| {
            row.objectForKey(key)
                .and_then(|value| value.downcast::<NSNumber>().ok())
                .map(|value| value.as_i64())
        };
        let string = |row: &NSDictionary<NSString, AnyObject>, key: &NSString| {
            row.objectForKey(key)
                .and_then(|value| value.downcast::<NSString>().ok())
                .map(|value| value.to_string())
                .unwrap_or_default()
        };
        list.iter()
            .filter_map(|row| {
                Some(WindowEntry {
                    number: u32::try_from(number(&row, &number_key)?).ok()?,
                    process_id: u32::try_from(number(&row, &pid_key)?).ok()?,
                    layer: number(&row, &layer_key).unwrap_or(0),
                    owner: string(&row, &owner_key),
                    name: string(&row, &name_key),
                })
            })
            .collect()
    })
}

pub fn list_windows() -> Result<Vec<TargetWindow>, String> {
    let own = std::process::id();
    let mut seen = HashSet::new();
    let mut windows: Vec<TargetWindow> = window_entries(true)
        .into_iter()
        .filter(|entry| entry.layer == 0 && entry.process_id != own && !entry.owner.is_empty())
        .filter(|entry| seen.insert(entry.number))
        .map(|entry| TargetWindow {
            handle: entry.number as isize,
            process_id: entry.process_id,
            title: if entry.name.trim().is_empty() || entry.name == entry.owner {
                entry.owner
            } else {
                format!("{} — {}", entry.name, entry.owner)
            },
        })
        .collect();
    windows.sort_by_key(|window| window.title.to_lowercase());
    Ok(windows)
}

fn frontmost_process() -> Option<u32> {
    autoreleasepool(|_| {
        let application = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        u32::try_from(application.processIdentifier()).ok()
    })
}

pub fn is_foreground(target: &TargetWindow) -> bool {
    if target.handle <= 0 || frontmost_process() != Some(target.process_id) {
        return false;
    }
    // The app is frontmost. Its front-most ordinary window must be the target,
    // which also tells EarControl's own main window and practice deck apart.
    let entries = window_entries(true);
    if !entries.iter().any(|entry| {
        entry.number as isize == target.handle && entry.process_id == target.process_id
    }) {
        return false;
    }
    entries
        .iter()
        .find(|entry| entry.process_id == target.process_id && entry.layer == 0)
        .is_none_or(|entry| entry.number as isize == target.handle)
}

// ---------------------------------------------------------------------------
// Devices

pub fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    // system_profiler reads cached pairing data. It never scans or connects.
    let output = std::process::Command::new("/usr/sbin/system_profiler")
        .args(["SPBluetoothDataType", "-json", "-detailLevel", "basic"])
        .output()
        .map_err(|error| format!("Could not read paired Bluetooth devices: {error}"))?;
    if !output.status.success() {
        return Err("Could not read paired Bluetooth devices.".into());
    }
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Could not read paired Bluetooth devices: {error}"))?;
    Ok(audio_devices(&report))
}

fn audio_devices(report: &serde_json::Value) -> Vec<DeviceInfo> {
    let mut devices = Vec::new();
    let mut seen = HashSet::new();
    let controllers = report["SPBluetoothDataType"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for controller in &controllers {
        for (group, connected) in [("device_connected", true), ("device_not_connected", false)] {
            for entry in controller[group].as_array().into_iter().flatten() {
                let Some(entry) = entry.as_object() else {
                    continue;
                };
                for (name, details) in entry {
                    let minor = details["device_minorType"].as_str().unwrap_or_default();
                    let lower = minor.to_lowercase();
                    let kind = if lower.contains("headphone") {
                        "Headphones"
                    } else if lower.contains("headset") || lower.contains("hands-free") {
                        "Headset"
                    } else if lower.contains("speaker") {
                        "Speaker"
                    } else if lower.contains("audio") {
                        "Bluetooth audio"
                    } else {
                        continue;
                    };
                    let address = details["device_address"].as_str().unwrap_or(name);
                    let id = format!(
                        "bluetooth:{}",
                        address.replace([':', '-'], "").to_uppercase()
                    );
                    if !seen.insert(id.clone()) {
                        continue;
                    }
                    devices.push(DeviceInfo {
                        id,
                        name: if name.trim().is_empty() {
                            "Bluetooth audio device".into()
                        } else {
                            name.clone()
                        },
                        connected,
                        transport: "Bluetooth".into(),
                        kind: kind.into(),
                    });
                }
            }
        }
    }
    devices.sort_by(|left, right| {
        right
            .connected
            .cmp(&left.connected)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    devices
}

pub fn open_bluetooth_settings() -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.BluetoothSettings")
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Bluetooth settings: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_profile_keys_to_mac_key_codes() {
        assert_eq!(mac_key(0x27), Some((0x7C, 0)));
        assert_eq!(mac_key(0x41), Some((0x00, 0)));
        assert_eq!(mac_key(0x5A), Some((0x06, 0)));
        assert_eq!(mac_key(0x30), Some((0x1D, 0)));
        assert_eq!(mac_key(0x74), Some((0x60, 0)));
        assert_eq!(mac_key(0x11), Some((0x3B, FLAG_CONTROL)));
        assert_eq!(mac_key(0x5B), Some((0x37, FLAG_COMMAND)));
        assert_eq!(mac_key(0x84), None);
        for key in crate::shortcuts::parse_shortcut("Ctrl+Alt+Shift+Win+F20").unwrap() {
            assert!(mac_key(key).is_some());
        }
    }

    #[test]
    fn media_keys_map_to_the_same_triggers_as_windows() {
        assert_eq!(trigger_for_media_key(16), Some("play_pause"));
        assert_eq!(trigger_for_media_key(17), Some("next"));
        assert_eq!(trigger_for_media_key(18), Some("previous"));
        assert_eq!(trigger_for_media_key(0), Some("volume_up"));
        assert_eq!(trigger_for_media_key(2), None);
    }

    #[test]
    fn consumed_media_keys_stay_consumed_until_released() {
        let mut keys = PressedKeys::default();
        keys.down(16, 1, true);
        assert_eq!(keys.repeat(16, 1, true), Some(true));
        assert_eq!(keys.repeat(16, 2, false), Some(true));
        assert!(keys.up(16));
        assert_eq!(keys.repeat(16, 1, true), None);
    }

    #[test]
    fn lists_only_audio_devices_connected_first() {
        let report = serde_json::json!({ "SPBluetoothDataType": [{
            "device_connected": [{ "Keyboard": { "device_address": "AA:BB", "device_minorType": "Keyboard" } }],
            "device_not_connected": [
                { "Zeta Buds": { "device_address": "11:22:33:44:55:66", "device_minorType": "Headset" } },
                { "Alpha": { "device_address": "66:55", "device_minorType": "Headphones" } }
            ]
        }]});
        let devices = audio_devices(&report);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Alpha");
        assert_eq!(devices[1].id, "bluetooth:112233445566");
        assert_eq!(devices[1].kind, "Headset");
        assert!(!devices[1].connected);
    }

    #[test]
    fn lists_windows_without_including_earcontrol() {
        let own = std::process::id();
        assert!(list_windows()
            .unwrap()
            .iter()
            .all(|window| window.process_id != own));
    }
}
