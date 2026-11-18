// Each platform exposes the same functions and types to the rest of the app.
#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(target_os = "macos", path = "macos.rs")]
mod platform;
pub use platform::*;
