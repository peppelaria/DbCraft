//! DbCraft: a desktop database client (MySQL/MariaDB, PostgreSQL, SQL Server, SQLite).

// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icons;
mod backend;
mod cli;
mod dialogs;
mod export;
mod models;
mod store;
mod tabs;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("DbCraft").with_inner_size([1280.0, 800.0]).with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DbCraft",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Light);
            match app::App::new(cc) {
                Ok(a) => Ok(Box::new(a) as Box<dyn eframe::App>),
                Err(e) => Err(e.into()),
            }
        }),
    )
}
