//! Command line: `dbcraft [file.db | url] [--open-table T] [--query SQL] [--new-connection KIND [--ssh-tab]]
//! [--dark] [--screenshot out.png [--delay S]]`.
//! The last three exist for scripted checks of the UI (the PNG comes from egui's framebuffer).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
pub struct Cli {
    pub db: Option<String>,
    pub open_table: Option<String>,
    pub query: Option<String>,
    pub screenshot: Option<PathBuf>,
    pub delay: f64,
    pub dark: bool,
    /// Opens the new-connection dialog for this engine (`mysql`, `postgres`, …).
    pub new_connection: Option<String>,
    /// …on its SSH tab.
    pub ssh_tab: bool,
    pub ssl_tab: bool,
    pub http_tab: bool,
    /// Table-view panels to open with `--open-table` (comma list: filter, cell, form, text, hide, tx).
    pub table_ui: Vec<String>,
    /// Opens the query builder: `tables;fields`, e.g. `customers,orders;customers.city,sum:orders.total`.
    pub builder: Option<String>,
    /// Query editor text, `|` = cursor, trailing `!` = open completion: `"SELECT u.| FROM users u!"`.
    pub editor: Option<String>,
    pub icon_gallery: bool,
    /// Opens a tool tab: users, backup, automation, model, queries, chat, ai-settings, others:<kind>, menu:<name>.
    pub open: Option<String>,
    /// Text in the tree's search box.
    pub tree_filter: Option<String>,
    started: Option<Instant>,
    shot_requested: bool,
}

impl Cli {
    pub fn parse() -> Cli {
        let mut c = Cli { delay: 3.0, ..Default::default() };
        let mut args = std::env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--open-table" => c.open_table = args.next(),
                "--query" => c.query = args.next(),
                "--screenshot" => c.screenshot = args.next().map(PathBuf::from),
                "--delay" => c.delay = args.next().and_then(|d| d.parse().ok()).unwrap_or(3.0),
                "--dark" => c.dark = true,
                "--new-connection" => c.new_connection = args.next(),
                "--ssh-tab" => c.ssh_tab = true,
                "--ssl-tab" => c.ssl_tab = true,
                "--http-tab" => c.http_tab = true,
                "--builder" => c.builder = args.next(),
                "--editor" => c.editor = args.next(),
                "--icon-gallery" => c.icon_gallery = true,
                "--open" => c.open = args.next(),
                "--tree-filter" => c.tree_filter = args.next(),
                "--table-ui" => c.table_ui = args.next().map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default(),
                _ if !a.starts_with('-') && c.db.is_none() => c.db = Some(a),
                _ => {}
            }
        }
        c
    }

    /// Takes the screenshot once the delay has passed, then closes the window.
    pub fn tick(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        let started = *self.started.get_or_insert_with(Instant::now);
        if self.shot_requested {
            return;
        }
        if started.elapsed().as_secs_f64() < self.delay {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        self.shot_requested = true;
        let ctx2 = ctx.clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img: Arc<egui::ColorImage>| {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
            match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                Some(buf) => {
                    if let Err(e) = buf.save(&path) {
                        eprintln!("screenshot: {e}");
                    }
                }
                None => eprintln!("screenshot: bad buffer"),
            }
            ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx2.request_repaint();
        })));
        ctx.request_repaint();
    }
}
