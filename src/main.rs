use anyhow::Result;
use eframe::egui;
use ghost_ai::{app, logging};

fn main() -> Result<()> {
    logging::init_logging();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let handle = runtime.handle().clone();
    let _guard = runtime.enter();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(900.0, 640.0))
            .with_always_on_top()
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(true),
        follow_system_theme: false,
        centered: true,
        ..eframe::NativeOptions::default()
    };

    let result = eframe::run_native(
        "Ghost AI",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::GhostApp::new(cc, handle.clone())))),
    );

    if let Err(err) = result {
        log::error!("eframe failed: {err}");
    }

    runtime.shutdown_background();
    Ok(())
}
