use anyhow::Result;
use eframe::egui::IconData;

/// Window / taskbar icon (all platforms).
pub fn window_icon() -> Result<IconData> {
    let bytes = include_bytes!("../../../assets/icons/app/window_icon.png");
    let image = image::load_from_memory(bytes)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
