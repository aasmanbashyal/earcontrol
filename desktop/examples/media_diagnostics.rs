//! Read-only aid for a physical-headphone test. Run while Monitor is active.
//! Does not print other applications' names, media titles or other personal data.
use windows::{
    Media::Control::{
        GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    },
    Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
};

fn is_earcontrol(
    session: &GlobalSystemMediaTransportControlsSession,
) -> windows::core::Result<bool> {
    let properties = session.TryGetMediaPropertiesAsync()?.join()?;
    Ok(properties.Title()? == "EarControl" && properties.Artist()? == "Headphone remote")
}

fn inspect() -> windows::core::Result<()> {
    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
    let sessions = manager.GetSessions()?;
    let total = sessions.Size()?;
    let mut earcontrol = 0;
    let mut unreadable = 0;
    for session in sessions {
        match is_earcontrol(&session) {
            Ok(true) => {
                earcontrol += 1;
                // This is EarControl's own source identity, never another app's metadata.
                println!("EarControl source ID: {}", session.SourceAppUserModelId()?);
            }
            Ok(false) => {}
            Err(_) => unreadable += 1,
        }
    }
    let selected = match manager.GetCurrentSession() {
        Ok(session) => match is_earcontrol(&session) {
            Ok(true) => "EarControl",
            Ok(false) => "another application",
            Err(_) => "unavailable",
        },
        Err(_) => "none or unavailable",
    };
    println!("EarControl media sessions: {earcontrol}");
    println!(
        "Other media sessions: {}",
        total.saturating_sub(earcontrol + unreadable)
    );
    println!("Unreadable sessions: {unreadable}");
    println!("Windows current media session: {selected}");
    if earcontrol == 0 {
        println!("Start Monitor in EarControl. If it is already active, check its startup warning and restart Monitor.");
    } else if earcontrol > 1 {
        println!("More than one EarControl session is registered. Stop or close the older copy before testing.");
    } else if selected != "EarControl" {
        println!("Windows currently selects a different session. Pause competing players and select EarControl in Windows media controls, then retry the earbuds.");
    } else {
        println!("Windows selects EarControl. Try a physical headphone gesture and check Button check or Activity.");
    }
    println!("This check reads Windows routing state; it does not test a physical headphone or send media commands.");
    Ok(())
}

fn main() -> windows::core::Result<()> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    let result = inspect();
    unsafe {
        RoUninitialize();
    }
    result
}
