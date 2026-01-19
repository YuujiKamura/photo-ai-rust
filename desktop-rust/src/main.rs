mod app;
mod io;
mod model;

use app::{configure_fonts, DesktopApp};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Photo AI Result Viewer",
        options,
        Box::new(|cc| {
            configure_fonts(&cc.egui_ctx);
            Box::new(DesktopApp::default())
        }),
    )
}
