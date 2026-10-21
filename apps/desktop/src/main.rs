// No console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("cannot start the async runtime: {e}");
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DayZ Community Hub")
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([960.0, 600.0])
            .with_decorations(false),
        ..Default::default()
    };
    let handle = runtime.handle().clone();
    eframe::run_native(
        "DayZ Community Hub",
        options,
        Box::new(move |cc| {
            dzch_ui::rt::Rt::install(handle, cc.egui_ctx.clone());
            Ok(Box::new(dzch_ui::HubApp::new(&cc.egui_ctx)))
        }),
    )
}
