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
        initial_window_size: Some(egui::vec2(900.0, 640.0)),
        follow_system_theme: true,
        centered: true,
        ..eframe::NativeOptions::default()
    };

    let result = eframe::run_native(
        "Ghost AI",
        native_options,
        Box::new(move |cc| Box::new(app::GhostApp::new(cc, handle.clone()))),
    );

    if let Err(err) = result {
        log::error!("eframe failed: {err}");
    }

    runtime.shutdown_background();
    Ok(())
}
