//! On macOS, eframe's AccessKit integration already serves the practice
//! viewport, so the deck needs no extra platform bridge.

pub struct Bridge;

impl Bridge {
    pub fn attach(_ctx: &eframe::egui::Context, _handle: isize) -> Result<Self, String> {
        Ok(Self)
    }
}
