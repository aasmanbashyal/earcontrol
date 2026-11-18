use crate::model::TargetWindow;
use std::path::PathBuf;
use windows::{
    core::{w, BOOL, PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HINSTANCE, HWND, LPARAM,
            LRESULT, WPARAM,
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{
                CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, EnumWindows, FindWindowW,
            GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible, RegisterClassW,
            SetForegroundWindow, ShowWindow, UnregisterClassW, SW_RESTORE, WINDOW_EX_STYLE,
            WM_COPYDATA, WNDCLASSW, WS_OVERLAPPED,
        },
    },
};

// Preserve the 2.x single-instance identity. A separate versioned mutex permits
// the old and new apps to register competing media sessions on the same desktop.
const INSTANCE_MUTEX: PCWSTR = w!("app.earcontrol.desktop-sim");
const INSTANCE_CLASS: PCWSTR = w!("app.earcontrol.desktop-sic");
const INSTANCE_WINDOW: PCWSTR = w!("app.earcontrol.desktop-siw");

pub struct SingleInstance {
    mutex: HANDLE,
    receiver: HWND,
    instance: HINSTANCE,
}
impl SingleInstance {
    pub fn acquire() -> Result<Option<Self>, windows::core::Error> {
        let Some(mutex) = acquire_mutex(INSTANCE_MUTEX)? else {
            // A concurrent launch may publish the mutex before its window exists.
            for _ in 0..40 {
                if focus_existing_instance() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            return Ok(None);
        };
        unsafe {
            let instance = match GetModuleHandleW(None) {
                Ok(module) => HINSTANCE(module.0),
                Err(error) => {
                    let _ = CloseHandle(mutex);
                    return Err(error);
                }
            };
            let class = WNDCLASSW {
                lpfnWndProc: Some(instance_window_proc),
                hInstance: instance,
                lpszClassName: INSTANCE_CLASS,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                let error = windows::core::Error::from_thread();
                let _ = CloseHandle(mutex);
                return Err(error);
            }
            match CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                INSTANCE_CLASS,
                INSTANCE_WINDOW,
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            ) {
                Ok(receiver) => Ok(Some(Self {
                    mutex,
                    receiver,
                    instance,
                })),
                Err(error) => {
                    let _ = UnregisterClassW(INSTANCE_CLASS, Some(instance));
                    let _ = CloseHandle(mutex);
                    Err(error)
                }
            }
        }
    }
}
impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.receiver);
            let _ = UnregisterClassW(INSTANCE_CLASS, Some(self.instance));
            let _ = CloseHandle(self.mutex);
        }
    }
}

fn acquire_mutex(name: PCWSTR) -> windows::core::Result<Option<HANDLE>> {
    unsafe {
        let handle = CreateMutexW(None, false, name)?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            Ok(None)
        } else {
            Ok(Some(handle))
        }
    }
}

// The old Tauri single-instance plugin sends WM_COPYDATA to this exact receiver.
// EarControl has no command-line actions to forward; receipt only restores its UI.
unsafe extern "system" fn instance_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_COPYDATA {
        let _ = std::panic::catch_unwind(|| focus_main_window(std::process::id()));
        return LRESULT(1);
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn focus_existing_instance() -> bool {
    unsafe {
        let Ok(receiver) = FindWindowW(INSTANCE_CLASS, INSTANCE_WINDOW) else {
            return false;
        };
        let mut pid = 0;
        GetWindowThreadProcessId(receiver, Some(&mut pid));
        if !is_earcontrol_process(pid) {
            return false;
        }
        focus_main_window(pid)
    }
}

fn is_earcontrol_process(pid: u32) -> bool {
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        if result.is_err() {
            return false;
        }
        let path = PathBuf::from(String::from_utf16_lossy(&path[..length as usize]));
        path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .eq_ignore_ascii_case("earcontrol.exe")
        })
    }
}

fn is_main_title(title: &str) -> bool {
    title == "EarControl"
        || (title.starts_with("EarControl · ") && !title.to_lowercase().contains("practice"))
}

fn focus_main_window(process_id: u32) -> bool {
    struct Search {
        process_id: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn find(hwnd: HWND, parameter: LPARAM) -> BOOL {
        let search = unsafe { &mut *(parameter.0 as *mut Search) };
        let mut owner = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner));
        }
        if owner != search.process_id || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut title = [0u16; 256];
        let length = unsafe { GetWindowTextW(hwnd, &mut title) } as usize;
        if is_main_title(&String::from_utf16_lossy(&title[..length])) {
            search.found = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }
    let mut search = Search {
        process_id,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(find), LPARAM(&mut search as *mut Search as isize));
        if let Some(hwnd) = search.found {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
            return true;
        }
    }
    false
}

pub fn settings_path() -> Result<PathBuf, String> {
    // An explicit path isolates tests and benchmarks from personal profiles.
    if let Some(path) = std::env::var_os("EARCONTROL_SETTINGS_PATH") {
        return Ok(PathBuf::from(path));
    }
    let roaming = std::env::var_os("APPDATA").ok_or("Windows APPDATA is unavailable.")?;
    Ok(PathBuf::from(roaming)
        .join("app.earcontrol.desktop")
        .join("profiles.json"))
}

pub fn target_exists(target: &TargetWindow) -> bool {
    if target.handle == 0 || target.process_id == 0 {
        return false;
    }
    unsafe {
        let window = HWND(target.handle as *mut _);
        let mut pid = 0;
        if !IsWindow(Some(window)).as_bool() {
            return false;
        }
        GetWindowThreadProcessId(window, Some(&mut pid));
        pid == target.process_id
    }
}

pub fn practice_target() -> Option<TargetWindow> {
    unsafe {
        let hwnd = FindWindowW(None, w!("EarControl practice deck")).ok()?;
        let target = TargetWindow {
            handle: hwnd.0 as isize,
            process_id: std::process::id(),
            title: "EarControl practice deck".into(),
        };
        target_exists(&target).then_some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::HSTRING;

    #[test]
    fn instance_identity_matches_the_legacy_plugin() {
        assert_eq!(
            unsafe { INSTANCE_MUTEX.to_string() }.unwrap(),
            "app.earcontrol.desktop-sim"
        );
        assert_eq!(
            unsafe { INSTANCE_CLASS.to_string() }.unwrap(),
            "app.earcontrol.desktop-sic"
        );
        assert_eq!(
            unsafe { INSTANCE_WINDOW.to_string() }.unwrap(),
            "app.earcontrol.desktop-siw"
        );
    }

    #[test]
    fn instance_lock_blocks_duplicates_and_releases_after_close() {
        let wide_name = HSTRING::from(format!(
            "EarControlMutexTest-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let name = PCWSTR(wide_name.as_ptr());
        let first = acquire_mutex(name).unwrap().unwrap();
        assert!(acquire_mutex(name).unwrap().is_none());
        unsafe {
            CloseHandle(first).unwrap();
        }
        let next = acquire_mutex(name).unwrap().unwrap();
        unsafe {
            CloseHandle(next).unwrap();
        }
    }

    #[test]
    fn legacy_and_native_main_titles_exclude_practice_and_input_windows() {
        assert!(is_main_title("EarControl"));
        assert!(is_main_title("EarControl · Buttons"));
        assert!(is_main_title("EarControl · Settings"));
        assert!(!is_main_title("EarControl practice deck"));
        assert!(!is_main_title("EarControl · Practice deck"));
        assert!(!is_main_title("EarControl input"));
        assert!(!is_main_title("Another app"));
    }
}
