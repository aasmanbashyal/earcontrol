//! Read-only aid for a physical-headphone test. Run while Monitor is active.
//! Windows only: it inspects Windows media sessions and the media-key hook.
#[cfg(windows)]
mod diagnostics;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    diagnostics::main()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("media_diagnostics inspects Windows media sessions and runs only on Windows.");
}
