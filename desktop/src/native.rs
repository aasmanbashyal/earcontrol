//! Windows media capture and input delivery. No Bluetooth pairing or audio playback.
use crate::model::{DeviceInfo, TargetWindow};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    mem::size_of,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    core::{factory, w, BOOL, HSTRING},
    Foundation::TypedEventHandler,
    Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    },
    Win32::{
        Devices::Bluetooth::{
            BluetoothFindDeviceClose, BluetoothFindFirstDevice, BluetoothFindNextDevice,
            BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS,
        },
        Foundation::{ERROR_NO_MORE_ITEMS, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::GetCurrentThreadId,
            WinRT::{
                ISystemMediaTransportControlsInterop, RoInitialize, RoUninitialize,
                RO_INIT_MULTITHREADED,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
                KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{
                CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
                EnumWindows, GetForegroundWindow, GetMessageW, GetWindowTextLengthW,
                GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, PeekMessageW,
                PostThreadMessageW, RegisterClassW, SetWindowsHookExW, TranslateMessage,
                UnhookWindowsHookEx, UnregisterClassW, DBT_DEVNODES_CHANGED, KBDLLHOOKSTRUCT,
                LLKHF_INJECTED, MSG, PBT_APMRESUMEAUTOMATIC, PM_NOREMOVE, WH_KEYBOARD_LL,
                WINDOW_EX_STYLE, WM_APP, WM_DEVICECHANGE, WM_KEYDOWN, WM_KEYUP, WM_POWERBROADCAST,
                WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW, WS_OVERLAPPED,
            },
        },
    },
};

const WAKE_INPUT_THREAD: u32 = WM_APP + 0x51;
type CommandCallback = Arc<dyn Fn(String, String, u64, Instant) + Send + Sync>;
type ConsumeCallback = Arc<dyn Fn(&str) -> bool + Send + Sync>;

#[derive(Default)]
struct SessionState {
    listening: AtomicBool,
    keyboard_fallback: AtomicBool,
    generation: AtomicU64,
}

impl SessionState {
    fn invalidate(&self) {
        self.listening.store(false, Ordering::SeqCst);
        self.keyboard_fallback.store(false, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

struct CapturedCommand {
    generation: u64,
    trigger: &'static str,
    source: &'static str,
    captured_at: Instant,
}

enum InputRequest {
    Start(bool, Sender<Result<(), String>>),
    Stop(Sender<Result<(), String>>),
    Quit,
}

struct HookState {
    session: Arc<SessionState>,
    events: SyncSender<CapturedCommand>,
    should_consume: ConsumeCallback,
    // A consumed down always has a consumed up, even after Stop.
    pressed: PressedKeys,
}

#[derive(Default)]
struct PressedKeys(HashMap<u32, (Option<u64>, bool)>);

impl PressedKeys {
    fn repeat(&self, key: u32, generation: u64) -> Option<bool> {
        self.0
            .get(&key)
            .filter(|(seen, _)| *seen == Some(generation))
            .map(|(_, consumed)| *consumed)
    }

    fn down(&mut self, key: u32, generation: u64, consumed: bool) {
        self.0.insert(key, (Some(generation), consumed));
    }

    fn up(&mut self, key: u32) -> bool {
        self.0.remove(&key).is_some_and(|(_, consumed)| consumed)
    }

    fn recover(&mut self) {
        // A removed device may never send key-up. Forget repeat suppression,
        // while retaining the release decision for any key-up still in flight.
        for (generation, _) in self.0.values_mut() {
            *generation = None;
        }
    }
}

thread_local! {
    static KEYBOARD_HOOK: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

// A hidden top-level window receives device-change broadcasts. Message-only
// windows do not receive these broadcasts, and the UI lives on another thread.
struct DeviceNotifications {
    hwnd: HWND,
    instance: HINSTANCE,
    class_name: HSTRING,
}

impl DeviceNotifications {
    fn new() -> Result<Self, String> {
        let instance = HINSTANCE(
            unsafe { GetModuleHandleW(None) }
                .map_err(|error| error.to_string())?
                .0,
        );
        let class_name = HSTRING::from(format!("EarControlInputEvents-{}", unsafe {
            GetCurrentThreadId()
        }));
        let class = WNDCLASSW {
            lpfnWndProc: Some(device_notification_proc),
            hInstance: instance,
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err(format!(
                "Could not watch input device changes: {}",
                windows::core::Error::from_thread()
            ));
        }
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                &class_name,
                w!("EarControl input"),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )
        };
        match window {
            Ok(hwnd) => Ok(Self {
                hwnd,
                instance,
                class_name,
            }),
            Err(error) => {
                unsafe {
                    let _ = UnregisterClassW(&class_name, Some(instance));
                }
                Err(format!("Could not watch input device changes: {error}"))
            }
        }
    }
}

impl Drop for DeviceNotifications {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = UnregisterClassW(&self.class_name, Some(self.instance));
        }
    }
}

unsafe extern "system" fn device_notification_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if (message == WM_DEVICECHANGE && wparam.0 as u32 == DBT_DEVNODES_CHANGED)
        || (message == WM_POWERBROADCAST && wparam.0 as u32 == PBT_APMRESUMEAUTOMATIC)
    {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            KEYBOARD_HOOK.with(|slot| {
                if let Some(state) = slot.borrow_mut().as_mut() {
                    state.pressed.recover();
                }
            });
        }));
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// The native thread owns all COM objects and the keyboard-hook message loop.
/// The handle contains only thread-safe channels and atomic session state.
pub struct NativeInput {
    requests: Sender<InputRequest>,
    thread_id: u32,
    session: Arc<SessionState>,
    native_thread: Option<JoinHandle<()>>,
}

impl NativeInput {
    pub fn new(
        hwnd: isize,
        on_command: CommandCallback,
        should_consume: ConsumeCallback,
    ) -> Result<Self, String> {
        if hwnd == 0 {
            return Err("The EarControl window has not been created yet.".into());
        }
        let session = Arc::new(SessionState::default());
        let (requests, receiver) = mpsc::channel();
        let (events, event_receiver) = mpsc::sync_channel::<CapturedCommand>(64);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let native_session = session.clone();
        let native_thread = thread::Builder::new()
            .name("earcontrol-windows-input".into())
            .spawn(move || {
                run_input_thread(
                    hwnd,
                    native_session,
                    receiver,
                    events,
                    should_consume,
                    ready_sender,
                );
            })
            .map_err(|error| error.to_string())?;

        let thread_id = match ready_receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(id)) => id,
            Ok(Err(error)) => {
                let _ = native_thread.join();
                return Err(error);
            }
            Err(error) => {
                return Err(format!(
                    "Windows media controls did not initialize: {error}"
                ))
            }
        };
        let callback_session = session.clone();
        // Do not run user callbacks inside a low-level keyboard hook.
        thread::Builder::new()
            .name("earcontrol-media-events".into())
            .spawn(move || {
                while let Ok(event) = event_receiver.recv() {
                    if callback_session.listening.load(Ordering::SeqCst)
                        && event.generation == callback_session.generation.load(Ordering::SeqCst)
                    {
                        let _ = catch_unwind(AssertUnwindSafe(|| {
                            on_command(
                                event.trigger.into(),
                                event.source.into(),
                                event.generation,
                                event.captured_at,
                            );
                        }));
                    }
                }
            })
            .map_err(|error| {
                let _ = requests.send(InputRequest::Quit);
                unsafe {
                    let _ = PostThreadMessageW(thread_id, WAKE_INPUT_THREAD, WPARAM(0), LPARAM(0));
                }
                error.to_string()
            })?;
        Ok(Self {
            requests,
            thread_id,
            session,
            native_thread: Some(native_thread),
        })
    }

    pub fn start(&self, keyboard_fallback: bool) -> Result<(), String> {
        self.session.invalidate();
        let (sender, receiver) = mpsc::channel();
        self.request(InputRequest::Start(keyboard_fallback, sender), receiver)
    }

    pub fn stop(&self) -> Result<(), String> {
        // Invalidate queued events before waiting on the COM thread.
        self.session.invalidate();
        let (sender, receiver) = mpsc::channel();
        self.request(InputRequest::Stop(sender), receiver)
    }

    /// Compare a callback's captured generation while accepting it into app state.
    /// This also closes the race where the callback worker pauses across Stop/start.
    pub fn generation(&self) -> u64 {
        self.session.generation.load(Ordering::SeqCst)
    }

    fn request(
        &self,
        request: InputRequest,
        response: Receiver<Result<(), String>>,
    ) -> Result<(), String> {
        self.requests
            .send(request)
            .map_err(|_| "Windows input thread has stopped.".to_string())?;
        unsafe {
            PostThreadMessageW(self.thread_id, WAKE_INPUT_THREAD, WPARAM(0), LPARAM(0))
                .map_err(|error| error.to_string())?;
        }
        match response.recv_timeout(Duration::from_secs(5)) {
            Ok(result) => result,
            Err(_) => {
                self.session.invalidate();
                Err("Windows media controls did not respond. Close and reopen EarControl.".into())
            }
        }
    }
}

impl Drop for NativeInput {
    fn drop(&mut self) {
        self.session.invalidate();
        let _ = self.requests.send(InputRequest::Quit);
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WAKE_INPUT_THREAD, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.native_thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_input_thread(
    hwnd: isize,
    session: Arc<SessionState>,
    requests: Receiver<InputRequest>,
    events: SyncSender<CapturedCommand>,
    should_consume: ConsumeCallback,
    ready: SyncSender<Result<u32, String>>,
) {
    let apartment = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    if let Err(error) = apartment {
        let _ = ready.send(Err(format!(
            "Could not initialize Windows media support: {error}"
        )));
        return;
    }
    let result = (|| -> Result<(), String> {
        let _device_notifications = DeviceNotifications::new()?;
        let interop: ISystemMediaTransportControlsInterop =
            factory::<SystemMediaTransportControls, _>().map_err(|error| error.to_string())?;
        let controls: SystemMediaTransportControls = unsafe {
            interop
                .GetForWindow(HWND(hwnd as *mut _))
                .map_err(|error| error.to_string())?
        };
        controls
            .SetIsEnabled(false)
            .map_err(|error| error.to_string())?;
        let callback_session = session.clone();
        let callback_events = events.clone();
        let token = controls
            .ButtonPressed(&TypedEventHandler::<
                SystemMediaTransportControls,
                SystemMediaTransportControlsButtonPressedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = args.as_ref() {
                    let trigger = match args.Button()? {
                        SystemMediaTransportControlsButton::Play
                        | SystemMediaTransportControlsButton::Pause => Some("play_pause"),
                        SystemMediaTransportControlsButton::Next => Some("next"),
                        SystemMediaTransportControlsButton::Previous => Some("previous"),
                        SystemMediaTransportControlsButton::Stop => Some("stop"),
                        _ => None,
                    };
                    if let Some(trigger) = trigger {
                        enqueue(
                            &callback_session,
                            &callback_events,
                            trigger,
                            "Windows media session",
                        );
                    }
                }
                Ok(())
            }))
            .map_err(|error| error.to_string())?;

        KEYBOARD_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(HookState {
                session: session.clone(),
                events,
                should_consume,
                pressed: PressedKeys::default(),
            });
        });
        // The fallback is optional. A failed keyboard-hook registration must not
        // prevent the user from starting the ordinary Windows media session.
        let mut hook = None;
        let mut message = MSG::default();
        unsafe {
            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        }
        let thread_id = unsafe { GetCurrentThreadId() };
        if ready.send(Ok(thread_id)).is_err() {
            // Initialization caller timed out; never leave an unowned hook running.
            let _ = controls.RemoveButtonPressed(token);
            KEYBOARD_HOOK.with(|slot| {
                slot.borrow_mut().take();
            });
            return Err("Windows media initialization was cancelled.".into());
        }

        let mut quit = false;
        while !quit {
            let got = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            if got <= 0 {
                break;
            }
            if message.message == WAKE_INPUT_THREAD {
                while let Ok(request) = requests.try_recv() {
                    match request {
                        InputRequest::Start(fallback, response) => {
                            session.invalidate();
                            let hook_result = if fallback && hook.is_none() {
                                unsafe { GetModuleHandleW(None) }.and_then(|module| unsafe {
                                    SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_callback), Some(HINSTANCE(module.0)), 0)
                                }).map(|registered| { hook = Some(registered); })
                                    .map_err(|error| format!("Could not register media keys. Turn off keyboard fallback and retry. {error}"))
                            } else {
                                Ok(())
                            };
                            let result = hook_result.and_then(|_| enable_controls(&controls));
                            if result.is_ok() {
                                session.keyboard_fallback.store(fallback, Ordering::SeqCst);
                                session.listening.store(true, Ordering::SeqCst);
                            } else {
                                let _ = disable_controls(&controls);
                            }
                            if response.send(result).is_err() {
                                session.invalidate();
                                let _ = disable_controls(&controls);
                            }
                        }
                        InputRequest::Stop(response) => {
                            session.invalidate();
                            let _ = response.send(disable_controls(&controls));
                        }
                        InputRequest::Quit => {
                            quit = true;
                            break;
                        }
                    }
                }
            } else {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        session.invalidate();
        let _ = disable_controls(&controls);
        let _ = controls.RemoveButtonPressed(token);
        if let Some(hook) = hook {
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
        KEYBOARD_HOOK.with(|slot| {
            slot.borrow_mut().take();
        });
        Ok(())
    })();
    if let Err(error) = result {
        let _ = ready.try_send(Err(error));
        session.invalidate();
        KEYBOARD_HOOK.with(|slot| {
            slot.borrow_mut().take();
        });
    }
    unsafe {
        RoUninitialize();
    }
}

fn enable_controls(controls: &SystemMediaTransportControls) -> Result<(), String> {
    (|| -> windows::core::Result<()> {
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsNextEnabled(true)?;
        controls.SetIsPreviousEnabled(true)?;
        controls.SetIsStopEnabled(true)?;
        let display = controls.DisplayUpdater()?;
        display.SetType(MediaPlaybackType::Music)?;
        display
            .MusicProperties()?
            .SetTitle(&HSTRING::from("EarControl"))?;
        display
            .MusicProperties()?
            .SetArtist(&HSTRING::from("Headphone remote"))?;
        display.Update()?;
        controls.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
        controls.SetIsEnabled(true)?;
        Ok(())
    })()
    .map_err(|error| error.to_string())
}

fn disable_controls(controls: &SystemMediaTransportControls) -> Result<(), String> {
    // Attempt both operations even if Windows rejects the first one.
    let enabled = controls.SetIsEnabled(false);
    let playback = controls.SetPlaybackStatus(MediaPlaybackStatus::Closed);
    enabled.and(playback).map_err(|error| error.to_string())
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

unsafe extern "system" fn keyboard_callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let consumed = catch_unwind(AssertUnwindSafe(|| {
            let input = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if input.flags.contains(LLKHF_INJECTED) {
                return false;
            }
            let Some(trigger) = trigger_for_key(input.vkCode) else {
                return false;
            };
            KEYBOARD_HOOK.with(|slot| {
                let mut state = slot.borrow_mut();
                let Some(state) = state.as_mut() else {
                    return false;
                };
                match wparam.0 as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => {
                        let generation = state.session.generation.load(Ordering::SeqCst);
                        if let Some(consumed) = state.pressed.repeat(input.vkCode, generation) {
                            return consumed;
                        }
                        let mut consume = false;
                        if state.session.listening.load(Ordering::SeqCst)
                            && state.session.keyboard_fallback.load(Ordering::SeqCst)
                        {
                            consume = (state.should_consume)(trigger);
                            let queued = enqueue(
                                &state.session,
                                &state.events,
                                trigger,
                                "Keyboard media key",
                            );
                            consume &= queued;
                        }
                        state.pressed.down(input.vkCode, generation, consume);
                        consume
                    }
                    WM_KEYUP | WM_SYSKEYUP => state.pressed.up(input.vkCode),
                    _ => false,
                }
            })
        }))
        .unwrap_or(false);
        if consumed {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn trigger_for_key(key: u32) -> Option<&'static str> {
    match key {
        0xAD => Some("mute"),
        0xAE => Some("volume_down"),
        0xAF => Some("volume_up"),
        0xB0 => Some("next"),
        0xB1 => Some("previous"),
        0xB2 => Some("stop"),
        0xB3 => Some("play_pause"),
        _ => None,
    }
}

pub fn list_windows() -> Result<Vec<TargetWindow>, String> {
    let mut windows = Vec::<TargetWindow>::new();
    unsafe {
        EnumWindows(
            Some(enumerate_window),
            LPARAM(&mut windows as *mut _ as isize),
        )
        .map_err(|error| error.to_string())?;
    }
    windows.sort_by_key(|window| window.title.to_lowercase());
    Ok(windows)
}

unsafe extern "system" fn enumerate_window(hwnd: HWND, parameter: LPARAM) -> BOOL {
    // All memory belongs to the synchronous EnumWindows call above.
    let windows = unsafe { &mut *(parameter.0 as *mut Vec<TargetWindow>) };
    if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return BOOL(1);
    }
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == 0 || process_id == std::process::id() {
        return BOOL(1);
    }
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return BOOL(1);
    }
    let mut title = vec![0u16; (length as usize + 1).min(4096)];
    let copied = unsafe { GetWindowTextW(hwnd, &mut title) };
    if copied > 0 {
        let title = String::from_utf16_lossy(&title[..copied as usize]);
        if !title.trim().is_empty() {
            windows.push(TargetWindow {
                handle: hwnd.0 as isize,
                process_id,
                title,
            });
        }
    }
    BOOL(1)
}

pub fn is_foreground(target: &TargetWindow) -> bool {
    let hwnd = HWND(target.handle as *mut _);
    if target.handle == 0
        || !unsafe { IsWindow(Some(hwnd)) }.as_bool()
        || unsafe { GetForegroundWindow() } != hwnd
    {
        return false;
    }
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    process_id != 0 && process_id == target.process_id
}

pub fn win_key_held() -> bool {
    unsafe { GetAsyncKeyState(0x5B) < 0 || GetAsyncKeyState(0x5C) < 0 }
}

pub fn modifiers_held() -> bool {
    [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(*key) } < 0)
}

fn keyboard_input(key: u16, scan: u16, released: bool, unicode: bool) -> INPUT {
    let mut flags = if released {
        KEYEVENTF_KEYUP
    } else {
        Default::default()
    };
    if unicode {
        flags |= KEYEVENTF_UNICODE;
    } else if matches!(key, 0x21..=0x28 | 0x2C..=0x2E | 0x5B | 0x5C | 0x6F | 0x90 | 0xA3 | 0xA5) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(key),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0x45415243,
            },
        },
    }
}

pub fn send_keys(keys: &[u16]) -> Result<(), String> {
    if keys.is_empty() {
        return Ok(());
    }
    if keys.len() > 5 || keys.iter().any(|key| *key == 0 || *key > 254) {
        return Err("The shortcut contains unsupported Windows keys.".into());
    }
    if modifiers_held() {
        return Err("Release Ctrl, Alt, Shift, and Windows before running a macro.".into());
    }
    let mut inputs = Vec::with_capacity(keys.len() * 2);
    inputs.extend(keys.iter().map(|key| keyboard_input(*key, 0, false, false)));
    inputs.extend(
        keys.iter()
            .rev()
            .map(|key| keyboard_input(*key, 0, true, false)),
    );
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;
    if sent != inputs.len() {
        let error = windows::core::Error::from_thread();
        if sent > 0 {
            let release: Vec<_> = keys
                .iter()
                .rev()
                .map(|key| keyboard_input(*key, 0, true, false))
                .collect();
            unsafe {
                SendInput(&release, size_of::<INPUT>() as i32);
            }
        }
        return Err(format!("Windows could not send the shortcut. Keep the target app at the same permission level as EarControl. {error}"));
    }
    Ok(())
}

/// Emits UTF-16 text directly. It never reads or replaces the clipboard.
pub fn send_text(text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    if text.chars().count() > 2000 {
        return Err("A text step can contain at most 2,000 characters.".into());
    }
    if modifiers_held() {
        return Err("Release Ctrl, Alt, Shift, and Windows before running a macro.".into());
    }
    let mut inputs = Vec::new();
    for code in text.encode_utf16() {
        inputs.push(keyboard_input(0, code, false, true));
        inputs.push(keyboard_input(0, code, true, true));
    }
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) } as usize;
    if sent != inputs.len() {
        let error = windows::core::Error::from_thread();
        // An odd partial count means the last Unicode key-down needs a release.
        if sent > 0 && sent % 2 == 1 {
            unsafe {
                SendInput(&inputs[sent..sent + 1], size_of::<INPUT>() as i32);
            }
        }
        return Err(format!(
            "Windows could not finish typing the macro text. {error}"
        ));
    }
    Ok(())
}

pub fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let parameters = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: BOOL(1),
        fReturnRemembered: BOOL(1),
        fReturnConnected: BOOL(1),
        // Cached devices only: no inquiry, pairing, or connection request.
        ..Default::default()
    };
    let mut information = BLUETOOTH_DEVICE_INFO {
        dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let search = match unsafe { BluetoothFindFirstDevice(&parameters, &mut information) } {
        Ok(search) => search,
        Err(error) if error.code() == windows::core::HRESULT::from_win32(ERROR_NO_MORE_ITEMS.0) => {
            return Ok(Vec::new())
        }
        Err(error) => return Err(format!("Could not read paired Bluetooth devices: {error}")),
    };
    let mut devices = Vec::new();
    let mut seen = HashSet::new();
    loop {
        let major_class = (information.ulClassofDevice >> 8) & 0x1f;
        if major_class == 4 {
            let address = unsafe { information.Address.Anonymous.ullLong };
            let id = format!("bluetooth:{address:012X}");
            if seen.insert(id.clone()) {
                let end = information
                    .szName
                    .iter()
                    .position(|character| *character == 0)
                    .unwrap_or(information.szName.len());
                let name = String::from_utf16_lossy(&information.szName[..end]);
                let minor_class = (information.ulClassofDevice >> 2) & 0x3f;
                let kind = match minor_class {
                    1 | 2 => "Headset",
                    6 => "Headphones",
                    5 => "Speaker",
                    _ => "Bluetooth audio",
                };
                devices.push(DeviceInfo {
                    id,
                    name: if name.trim().is_empty() {
                        "Bluetooth audio device".into()
                    } else {
                        name
                    },
                    connected: information.fConnected.as_bool(),
                    transport: "Bluetooth Classic".into(),
                    kind: kind.into(),
                });
            }
        }
        if unsafe { BluetoothFindNextDevice(search, &mut information) }.is_err() {
            break;
        }
    }
    unsafe {
        let _ = BluetoothFindDeviceClose(search);
    }
    devices.sort_by(|left, right| {
        right
            .connected
            .cmp(&left.connected)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(devices)
}

pub fn open_bluetooth_settings() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:bluetooth")
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not open Bluetooth settings: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    use windows::{
        core::w,
        Media::Control::GlobalSystemMediaTransportControlsSessionManager,
        Win32::Graphics::Gdi::{GetSysColorBrush, UpdateWindow, COLOR_WINDOW},
        Win32::UI::{
            Input::KeyboardAndMouse::SetFocus,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SendMessageW,
                SetForegroundWindow, ShowWindow, UnregisterClassW, PM_REMOVE, SW_SHOW,
                WINDOW_EX_STYLE, WM_CHAR, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            },
        },
    };

    thread_local! {
        static TEST_KEYS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
        static TEST_TEXT: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn test_window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_KEYDOWN {
            TEST_KEYS.with(|keys| keys.borrow_mut().push(wparam.0 as u32));
            return LRESULT(0);
        }
        if message == WM_CHAR {
            TEST_TEXT.with(|text| text.borrow_mut().push(wparam.0 as u16));
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    struct TestApartment;
    impl Drop for TestApartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }

    struct TestWindow {
        handle: HWND,
        instance: HINSTANCE,
    }
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.handle);
                let _ = UnregisterClassW(w!("EarControlNativeSmokeTest"), Some(self.instance));
            }
        }
    }

    fn pump_for(duration: Duration) {
        let start = Instant::now();
        while start.elapsed() < duration {
            let mut message = MSG::default();
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn native_input_has_windows_abi_size() {
        assert_eq!(
            size_of::<INPUT>(),
            if cfg!(target_pointer_width = "64") {
                40
            } else {
                28
            }
        );
    }

    #[test]
    fn only_headphone_media_keys_are_recognized() {
        assert_eq!(trigger_for_key(0xB3), Some("play_pause"));
        assert_eq!(trigger_for_key(0xAD), Some("mute"));
        assert_eq!(trigger_for_key(0x41), None);
        assert_eq!(trigger_for_key(0xB4), None);
    }

    #[test]
    fn held_media_keys_repeat_only_after_release() {
        let mut pressed = PressedKeys::default();
        assert_eq!(pressed.repeat(0xB3, 1), None);
        pressed.down(0xB3, 1, true);
        assert_eq!(pressed.repeat(0xB3, 1), Some(true));
        assert_eq!(pressed.repeat(0xB0, 1), None);
        assert!(pressed.up(0xB3));
        assert_eq!(pressed.repeat(0xB3, 1), None);
        assert!(!pressed.up(0xB3));
    }

    #[test]
    fn restarting_capture_recovers_a_missing_media_key_release() {
        let mut pressed = PressedKeys::default();
        pressed.down(0xB3, 1, true);
        assert_eq!(pressed.repeat(0xB3, 1), Some(true));
        assert_eq!(pressed.repeat(0xB3, 2), None);
        // A delayed release from the previous session still must be consumed.
        assert!(pressed.up(0xB3));
        pressed.down(0xB3, 2, false);
        assert_eq!(pressed.repeat(0xB3, 2), Some(false));
        assert!(!pressed.up(0xB3));
    }

    #[test]
    fn device_changes_recover_stuck_buttons_without_leaking_key_ups() {
        let mut pressed = PressedKeys::default();
        pressed.down(0xB0, 1, true);
        pressed.down(0xB3, 1, true);
        pressed.recover();
        assert_eq!(pressed.repeat(0xB0, 1), None);
        assert_eq!(pressed.repeat(0xB3, 1), None);
        assert!(pressed.up(0xB0));
        // A new device can send this button even if the old one never released it.
        pressed.down(0xB3, 1, true);
        assert_eq!(pressed.repeat(0xB3, 1), Some(true));
        assert!(pressed.up(0xB3));
    }

    #[test]
    fn device_and_resume_notifications_recover_media_capture() {
        let window = DeviceNotifications::new().expect("Create hidden device notification window");
        let (events, _receiver) = mpsc::sync_channel(1);
        KEYBOARD_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(HookState {
                session: Arc::new(SessionState::default()),
                events,
                should_consume: Arc::new(|_| false),
                pressed: PressedKeys::default(),
            });
        });
        for (message, event) in [
            (WM_DEVICECHANGE, DBT_DEVNODES_CHANGED),
            (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC),
        ] {
            KEYBOARD_HOOK.with(|slot| {
                slot.borrow_mut()
                    .as_mut()
                    .unwrap()
                    .pressed
                    .down(0xB3, 1, true)
            });
            unsafe {
                SendMessageW(
                    window.hwnd,
                    message,
                    Some(WPARAM(event as usize)),
                    Some(LPARAM(0)),
                );
            }
            KEYBOARD_HOOK.with(|slot| {
                let mut state = slot.borrow_mut();
                let pressed = &mut state.as_mut().unwrap().pressed;
                assert_eq!(pressed.repeat(0xB3, 1), None);
                assert!(pressed.up(0xB3));
            });
        }
        KEYBOARD_HOOK.with(|slot| {
            slot.borrow_mut().take();
        });
    }

    #[test]
    fn session_stop_invalidates_queued_work() {
        let session = SessionState::default();
        session.listening.store(true, Ordering::SeqCst);
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(enqueue(&session, &sender, "next", "test"));
        let event = receiver.recv().unwrap();
        session.invalidate();
        assert!(!session.listening.load(Ordering::SeqCst));
        assert_ne!(session.generation.load(Ordering::SeqCst), event.generation);
        assert!(!enqueue(&session, &sender, "next", "test"));
    }

    /// Runs against an isolated, foreground window and its own uniquely tagged SMTC session.
    /// It does not require earbuds and does not send commands to another app's media session.
    #[test]
    #[ignore = "opens a native test window and sends input; run separately with an unlocked desktop"]
    fn native_windows_smoke() {
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).expect("Initialize WinRT for smoke test");
        }
        let _apartment = TestApartment;
        TEST_KEYS.with(|keys| keys.borrow_mut().clear());
        TEST_TEXT.with(|text| text.borrow_mut().clear());
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None).unwrap() }.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(test_window_proc),
            hInstance: instance,
            lpszClassName: w!("EarControlNativeSmokeTest"),
            hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
            ..Default::default()
        };
        assert_ne!(
            unsafe { RegisterClassW(&class) },
            0,
            "Register test window class"
        );
        let handle =
            unsafe {
                CreateWindowExW(
                WINDOW_EX_STYLE::default(), w!("EarControlNativeSmokeTest"),
                w!("EarControl Windows test - keep this window focused; it closes automatically"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE, 120, 120, 720, 220, None, None,
                Some(instance), None,
            ).expect("Create isolated test window")
            };
        unsafe {
            let _ = ShowWindow(handle, SW_SHOW);
            let _ = UpdateWindow(handle);
        }
        let _window = TestWindow { handle, instance };
        let target = TargetWindow {
            handle: handle.0 as isize,
            process_id: std::process::id(),
            title: "Native smoke test".into(),
        };
        unsafe {
            let _ = SetForegroundWindow(handle);
            let _ = SetFocus(Some(handle));
        }
        pump_for(Duration::from_millis(100));
        println!(
            "Waiting for focus... activate the EarControl Windows test window within 45 seconds."
        );
        let focus_deadline = Instant::now() + Duration::from_secs(45);
        while !is_foreground(&target) && Instant::now() < focus_deadline {
            pump_for(Duration::from_millis(50));
        }
        assert!(
            is_foreground(&target),
            "The test window must be foreground; stop interacting with other windows."
        );
        pump_for(Duration::from_millis(200));
        TEST_KEYS.with(|keys| keys.borrow_mut().clear());
        TEST_TEXT.with(|text| text.borrow_mut().clear());
        assert!(!is_foreground(&TargetWindow {
            process_id: target.process_id + 1,
            ..target.clone()
        }));
        assert!(
            !modifiers_held(),
            "Release modifier keys before running the test."
        );
        let devices = list_devices().expect("Enumerate cached classic Bluetooth audio devices");
        println!(
            "PASS: paired Bluetooth audio enumeration returned {} device(s).",
            devices.len()
        );

        assert!(
            is_foreground(&target),
            "Refuse input because the test window lost focus"
        );
        send_keys(&[0x27]).expect("Send Right to own test window");
        assert!(
            is_foreground(&target),
            "Refuse input because the test window lost focus"
        );
        send_keys(&[0x25]).expect("Send Left to own test window");
        send_keys(&[]).expect("Disabled shortcut");
        pump_for(Duration::from_millis(150));
        TEST_KEYS.with(|keys| assert_eq!(*keys.borrow(), vec![0x27, 0x25]));
        println!(
            "PASS: real Windows Right/Left delivery, disabled mapping, and foreground identity."
        );

        assert!(is_foreground(&target));
        send_text("Hello 🦀").expect("Send Unicode to own test window");
        pump_for(Duration::from_millis(150));
        TEST_TEXT.with(|text| assert_eq!(String::from_utf16_lossy(&text.borrow()), "Hello 🦀"));
        println!("PASS: Unicode text including a surrogate pair reached the test window.");

        let (sender, received) = mpsc::channel::<(String, String, u64)>();
        let native = NativeInput::new(
            target.handle,
            Arc::new(move |trigger, source, generation, _captured_at| {
                let _ = sender.send((trigger, source, generation));
            }),
            Arc::new(|_| false),
        )
        .expect("Initialize Windows media capture");
        native.start(true).expect("Start media capture");
        let active_generation = native.generation();
        let interop: ISystemMediaTransportControlsInterop =
            factory::<SystemMediaTransportControls, _>().unwrap();
        let controls: SystemMediaTransportControls =
            unsafe { interop.GetForWindow(handle).unwrap() };
        let marker = format!(
            "earcontrol-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        controls
            .DisplayUpdater()
            .unwrap()
            .MusicProperties()
            .unwrap()
            .SetAlbumTitle(&HSTRING::from(&marker))
            .unwrap();
        controls.DisplayUpdater().unwrap().Update().unwrap();

        let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .unwrap()
            .join()
            .unwrap();
        let started = Instant::now();
        let session = loop {
            let mut found = None;
            for candidate in manager.GetSessions().unwrap() {
                if let Ok(properties) = candidate
                    .TryGetMediaPropertiesAsync()
                    .and_then(|operation| operation.join())
                {
                    if properties.Title().unwrap_or_default() == "EarControl"
                        && properties.AlbumTitle().unwrap_or_default() == marker.as_str()
                    {
                        found = Some(candidate);
                        break;
                    }
                }
            }
            if let Some(session) = found {
                break session;
            }
            assert!(
                started.elapsed() < Duration::from_secs(8),
                "Windows did not publish the test SMTC session."
            );
            pump_for(Duration::from_millis(100));
        };
        assert!(
            session.TrySkipNextAsync().unwrap().join().unwrap(),
            "Send Next to the uniquely tagged test session"
        );
        let event = received
            .recv_timeout(Duration::from_secs(3))
            .expect("Receive the real Windows media event");
        assert_eq!(
            event,
            (
                "next".into(),
                "Windows media session".into(),
                active_generation
            )
        );
        assert!(
            is_foreground(&target),
            "Refuse to send test input if another window became active"
        );
        let before = TEST_KEYS.with(|keys| keys.borrow().len());
        send_keys(&[0x27]).unwrap();
        pump_for(Duration::from_millis(150));
        TEST_KEYS.with(|keys| {
            assert_eq!(keys.borrow().len(), before + 1);
            assert_eq!(keys.borrow().last(), Some(&0x27));
        });
        assert!(
            received.try_recv().is_err(),
            "One media command must produce one event"
        );
        println!(
            "PASS: real SMTC Next event carries its session generation and produces one Right key."
        );

        native.stop().unwrap();
        assert_ne!(native.generation(), active_generation);
        assert!(!controls.IsEnabled().unwrap());
        assert_eq!(
            controls.PlaybackStatus().unwrap(),
            MediaPlaybackStatus::Closed
        );
        let _ = session
            .TrySkipNextAsync()
            .and_then(|operation| operation.join());
        pump_for(Duration::from_millis(200));
        assert!(
            received.try_recv().is_err(),
            "Stopped media session must not deliver more commands"
        );
        native.start(false).unwrap();
        assert_ne!(native.generation(), active_generation);
        native.stop().unwrap();
        drop(native);
        println!("PASS: Stop closes the session; restart changes generation.");

        // Exercise the actual application bridge as well as the low-level callback.
        // This catches generation, mode and repaint regressions in a UI framework port.
        let profiles = tempfile::tempdir().unwrap();
        let repaint_count = Arc::new(AtomicU64::new(0));
        let wake_count = Arc::clone(&repaint_count);
        let runtime =
            crate::runtime::AppState::new(profiles.path().join("profiles.json"), move || {
                wake_count.fetch_add(1, Ordering::Relaxed);
            });
        runtime.initialize(target.handle);
        assert!(runtime.warning().is_none(), "{:?}", runtime.warning());
        runtime.set_mode("monitor", None).unwrap();
        assert_eq!(runtime.status().mode, "monitor");
        let keys_before_monitor = TEST_KEYS.with(|keys| keys.borrow().len());
        let wakes_before_command = repaint_count.load(Ordering::Relaxed);
        assert!(session.TrySkipNextAsync().unwrap().join().unwrap());
        let deadline = Instant::now() + Duration::from_secs(3);
        while !runtime
            .events()
            .iter()
            .any(|event| event.trigger == "next" && event.outcome == "detected")
        {
            assert!(
                Instant::now() < deadline,
                "Monitor did not record the real Windows media event: {:?}",
                runtime.events()
            );
            pump_for(Duration::from_millis(20));
        }
        assert!(repaint_count.load(Ordering::Relaxed) > wakes_before_command);
        TEST_KEYS.with(|keys| assert_eq!(keys.borrow().len(), keys_before_monitor));
        runtime.stop().unwrap();
        assert_eq!(runtime.status().mode, "idle");
        runtime.clear_events();
        let _ = session
            .TrySkipNextAsync()
            .and_then(|operation| operation.join());
        pump_for(Duration::from_millis(200));
        assert!(
            runtime.events().is_empty(),
            "Stopped Monitor accepted a media event"
        );
        runtime.set_mode("monitor", None).unwrap();
        assert!(session.TryTogglePlayPauseAsync().unwrap().join().unwrap());
        let deadline = Instant::now() + Duration::from_secs(3);
        while !runtime
            .events()
            .iter()
            .any(|event| event.trigger == "play_pause" && event.outcome == "detected")
        {
            assert!(
                Instant::now() < deadline,
                "Restarted Monitor did not record Play/Pause"
            );
            pump_for(Duration::from_millis(20));
        }
        runtime.stop().unwrap();
        println!("PASS: application Monitor records real SMTC Next and Play/Pause, wakes the UI, sends no keys, and survives Stop/restart. Physical earbuds were not tested.");
    }
}
