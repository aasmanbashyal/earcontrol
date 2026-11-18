use crate::model::TargetWindow;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
use std::{
    io::{Read, Seek, Write},
    os::fd::AsRawFd,
    path::PathBuf,
};

const PRACTICE_TITLE: &str = "EarControl practice deck";
const LOCK_EXCLUSIVE: i32 = 2;
const LOCK_NONBLOCK: i32 = 4;

extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

/// Holds an advisory lock for the app's lifetime. A second launch activates
/// the running copy instead, matching the Windows single-instance behavior.
pub struct SingleInstance {
    _file: std::fs::File,
}

impl SingleInstance {
    pub fn acquire() -> Result<Option<Self>, std::io::Error> {
        let settings = settings_path().map_err(std::io::Error::other)?;
        let folder = settings
            .parent()
            .ok_or_else(|| std::io::Error::other("The settings folder is unavailable."))?;
        std::fs::create_dir_all(folder)?;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(folder.join("instance.lock"))?;
        if unsafe { flock(file.as_raw_fd(), LOCK_EXCLUSIVE | LOCK_NONBLOCK) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(35) {
                return Err(error);
            }
            let mut owner = String::new();
            let _ = file.read_to_string(&mut owner);
            if let Ok(pid) = owner.trim().parse::<i32>() {
                activate(pid);
            }
            return Ok(None);
        }
        file.set_len(0)?;
        file.rewind()?;
        write!(file, "{}", std::process::id())?;
        file.flush()?;
        Ok(Some(Self { _file: file }))
    }
}

fn activate(pid: i32) {
    objc2::rc::autoreleasepool(|_| {
        if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
            #[allow(deprecated)]
            app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        }
    });
}

pub fn settings_path() -> Result<PathBuf, String> {
    // An explicit path isolates tests and benchmarks from personal profiles.
    if let Some(path) = std::env::var_os("EARCONTROL_SETTINGS_PATH") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").ok_or("The macOS home folder is unavailable.")?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/app.earcontrol.desktop")
        .join("profiles.json"))
}

pub fn target_exists(target: &TargetWindow) -> bool {
    target.handle > 0
        && target.process_id != 0
        && crate::native::window_entries(false).iter().any(|entry| {
            entry.number as isize == target.handle && entry.process_id == target.process_id
        })
}

pub fn practice_target() -> Option<TargetWindow> {
    let own = std::process::id();
    // The deck starts hidden, so search every window, not just visible ones.
    crate::native::window_entries(false)
        .into_iter()
        .find(|entry| entry.process_id == own && entry.name == PRACTICE_TITLE)
        .map(|entry| TargetWindow {
            handle: entry.number as isize,
            process_id: own,
            title: PRACTICE_TITLE.into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_stay_in_the_shared_application_support_folder() {
        if std::env::var_os("EARCONTROL_SETTINGS_PATH").is_some() {
            return;
        }
        let path = settings_path().unwrap();
        assert!(path.ends_with("Library/Application Support/app.earcontrol.desktop/profiles.json"));
    }

    #[test]
    fn missing_windows_are_not_targets() {
        let target = TargetWindow {
            handle: 0,
            process_id: std::process::id(),
            title: PRACTICE_TITLE.into(),
        };
        assert!(!target_exists(&target));
    }
}
