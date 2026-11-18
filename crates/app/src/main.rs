//! DbCraft: a desktop database client (MySQL/MariaDB, PostgreSQL, SQL Server, SQLite).

// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icons;
mod backend;
mod cli;
mod dialogs;
mod dump;
#[cfg(test)]
mod servertest;
mod export;
mod fonts;
mod i18n;
mod models;
mod selfupdate;
mod store;
mod tabs;
mod update;
mod update_ui;

fn main() -> eframe::Result {
    // `--size 1440x900`: a fixed window size (scripted screenshots), not the remembered one
    let size = std::env::args().skip_while(|a| a != "--size").nth(1).and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("DbCraft").with_inner_size(size.unwrap_or([1280.0, 800.0])).with_min_inner_size([800.0, 500.0]),
        persist_window: size.is_none(),
        ..Default::default()
    };
    eframe::run_native(
        "DbCraft",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Light);
            // the language: `--lang xx` (scripted screenshots), else the setting, else the system's
            let arg = std::env::args().skip_while(|a| a != "--lang").nth(1);
            let setting = store::Settings::load().ui_lang;
            i18n::set_lang(i18n::resolve(arg.as_deref().unwrap_or(&setting)));
            fonts::apply(&cc.egui_ctx);
            match app::App::new(cc) {
                Ok(a) => Ok(Box::new(a) as Box<dyn eframe::App>),
                Err(e) => Err(e.into()),
            }
        }),
    )
}
