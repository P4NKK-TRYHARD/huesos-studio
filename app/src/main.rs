#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod canvas;
mod clip;
mod draw;
mod filedlg;
mod panels;
mod timeline;
mod mesh;
mod model;
mod pose;
mod props;
mod sample;
mod spine_io;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut screenshot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut opts: Vec<(String, String)> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else if let Some(k) = a.strip_prefix("--") {
            // opciones de arranque: --animacion N --fotograma F --hueso nombre --slot nombre ...
            let flag = matches!(k, "grafico" | "configurar" | "reproducir");
            let v = if flag { String::new() } else { args.next().unwrap_or_default() };
            opts.push((k.to_string(), v));
        } else {
            open = Some(PathBuf::from(a));
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Huesos Studio")
        .with_inner_size([1360.0, 820.0])
        .with_min_inner_size([900.0, 560.0]);
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };

    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|b| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            b.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "Huesos Studio",
        options,
        Box::new(move |cc| Ok(Box::new(app::EditorApp::new(cc, screenshot, open, opts)))),
    )
}
