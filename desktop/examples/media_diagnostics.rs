//! Read-only aid for a physical-headphone test. Run while Monitor is active.
//! `--watch-seconds 90` also observes media keys without consuming them.
//! Reports application identities, never media titles or ordinary keystrokes.
use std::{
    cell::RefCell,
    sync::mpsc::{self, SyncSender},
    thread,
    time::{Duration, Instant},
};
use windows::{
    Media::Control::GlobalSystemMediaTransportControlsSessionManager,
    Win32::{
        Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::GetCurrentThreadId,
            WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
        },
        UI::WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
            UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE, WH_KEYBOARD_LL,
            WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
        },
    },
};

fn source_label(source: &str) -> String {
    // Preserve enough identity to distinguish installed and portable copies,
    // while omitting the local account directory from diagnostic output.
    match std::env::var("USERPROFILE") {
        Ok(profile) if !profile.is_empty() => source.replace(&profile, "%USERPROFILE%"),
        _ => source.into(),
    }
}

fn inspect(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
) -> windows::core::Result<String> {
    let sessions = manager.GetSessions()?;
    let mut lines = Vec::new();
    for session in sessions {
        let source = source_label(&session.SourceAppUserModelId()?.to_string_lossy());
        let state = session.GetPlaybackInfo()?;
        let controls = state.Controls()?;
        lines.push(format!(
            "session source={source:?}; playback={:?}; play={}; pause={}; next={}; previous={}",
            state.PlaybackStatus()?,
            controls.IsPlayEnabled()?,
            controls.IsPauseEnabled()?,
            controls.IsNextEnabled()?,
            controls.IsPreviousEnabled()?
        ));
    }
    lines.sort();
    let current = manager
        .GetCurrentSession()
        .and_then(|session| session.SourceAppUserModelId())
        .map(|source| source_label(&source.to_string_lossy()))
        .unwrap_or_else(|_| "none or unavailable".into());
    lines.insert(0, format!("Windows current media session: {current:?}"));
    Ok(lines.join("\n"))
}

struct MediaKey {
    captured_at: Instant,
    code: u32,
    down: bool,
    injected: bool,
    earcontrol_tag: bool,
}

thread_local! {
    static MEDIA_KEYS: RefCell<Option<SyncSender<MediaKey>>> = const { RefCell::new(None) };
}

unsafe extern "system" fn observe_media_key(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        let input = unsafe { &*(data.0 as *const KBDLLHOOKSTRUCT) };
        if (0xAD..=0xB3).contains(&input.vkCode) {
            MEDIA_KEYS.with(|sender| {
                if let Some(sender) = sender.borrow().as_ref() {
                    let _ = sender.try_send(MediaKey {
                        captured_at: Instant::now(),
                        code: input.vkCode,
                        down: matches!(message.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN),
                        injected: input.flags.contains(LLKHF_INJECTED),
                        earcontrol_tag: input.dwExtraInfo == 0x45415243,
                    });
                }
            });
        }
    }
    // Always pass the event through. Never consume or synthesize a key.
    unsafe { CallNextHookEx(None, code, message, data) }
}

struct PassiveHook {
    thread: Option<thread::JoinHandle<()>>,
    id: u32,
}

impl PassiveHook {
    fn start(events: SyncSender<MediaKey>) -> Result<Self, String> {
        let (ready, receiver) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            let result = (|| -> windows::core::Result<()> {
                let module = unsafe { GetModuleHandleW(None) }?;
                let hook = unsafe {
                    SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(observe_media_key),
                        Some(HINSTANCE(module.0)),
                        0,
                    )
                }?;
                MEDIA_KEYS.with(|sender| *sender.borrow_mut() = Some(events));
                let mut message = MSG::default();
                unsafe {
                    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                }
                if ready.send(Ok(unsafe { GetCurrentThreadId() })).is_ok() {
                    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {}
                }
                unsafe {
                    let _ = UnhookWindowsHookEx(hook);
                }
                MEDIA_KEYS.with(|sender| {
                    sender.borrow_mut().take();
                });
                Ok(())
            })();
            if let Err(error) = result {
                let _ = ready.send(Err(error.to_string()));
            }
        });
        let id = receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())??;
        Ok(Self {
            thread: Some(thread),
            id,
        })
    }
}

impl Drop for PassiveHook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(seconds: u64) -> Result<(), Box<dyn std::error::Error>> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
    let (events, receiver) = mpsc::sync_channel(128);
    let _hook = if seconds > 0 {
        Some(PassiveHook::start(events)?)
    } else {
        None
    };
    let started = Instant::now();
    let deadline = started + Duration::from_secs(seconds);
    let mut previous = String::new();
    let mut next_snapshot = started;
    println!("Read-only routing/media-key observation. No media commands are sent; no keys are consumed.");
    println!("SMTC callbacks inside EarControl are visible in its Button check/Activity, not in this helper.");
    loop {
        if Instant::now() >= next_snapshot {
            let snapshot =
                inspect(&manager).unwrap_or_else(|error| format!("Routing query failed: {error}"));
            if snapshot != previous {
                println!("[{:.3}s] {snapshot}", started.elapsed().as_secs_f64());
                previous = snapshot;
            }
            next_snapshot = Instant::now() + Duration::from_secs(1);
        }
        while let Ok(event) = receiver.try_recv() {
            println!(
                "[{:.3}s] media key vk=0x{:02X}; down={}; injected={}; earcontrol_tag={}",
                event
                    .captured_at
                    .saturating_duration_since(started)
                    .as_secs_f64(),
                event.code,
                event.down,
                event.injected,
                event.earcontrol_tag
            );
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    println!(
        "Observation ended. A registered/current session does not prove physical-earbud delivery."
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let seconds = match args.as_slice() {
        [] => 0,
        [flag, value] if flag == "--watch-seconds" => value.parse::<u64>()?,
        _ => return Err("Usage: media_diagnostics [--watch-seconds 1..300]".into()),
    };
    if seconds > 300 {
        return Err("Observation is limited to 300 seconds.".into());
    }
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    let result = run(seconds);
    unsafe {
        RoUninitialize();
    }
    result
}
