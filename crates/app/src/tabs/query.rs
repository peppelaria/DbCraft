//! Query editor: SQL with syntax highlighting, run all / run selection, one result tab per
//! statement plus a message log.

use std::time::Instant;

use db_core::ResultSet;
use dbgrid::{DataGrid, GridColumn};
use egui::RichText;

use crate::app::{Action, Env, Location};
use crate::backend::{Msg, TabId};
use crate::dialogs::ExportSource;
use crate::models::{ResultModel, grid_columns};

struct ResultView {
    rs: ResultSet,
    columns: Vec<GridColumn>,
    grid: DataGrid,
}

pub struct QueryTab {
    pub id: TabId,
    pub loc: Location,
    sql: String,
    results: Vec<ResultView>,
    /// 0 = messages, i = results[i - 1].
    active_result: usize,
    messages: Vec<(String, bool)>,
    running: Option<Instant>,
    last: String,
    path: Option<std::path::PathBuf>,
    pending_run: bool,
}

impl QueryTab {
    pub fn new(id: TabId, loc: Location) -> Self {
        QueryTab { id, loc, sql: String::new(), results: Vec::new(), active_result: 0, messages: Vec::new(), running: None, last: String::new(), path: None, pending_run: false }
    }

    pub fn set_sql(&mut self, sql: String, run: bool) {
        self.sql = sql;
        self.pending_run = run;
    }

    fn run(&mut self, env: &mut Env<'_>, sql: String) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        if sql.trim().is_empty() || self.running.is_some() {
            return;
        }
        self.running = Some(Instant::now());
        self.messages.push((format!("> {}", sql.trim()), false));
        let tab = self.id;
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.execute(&sql).await,
                Err(e) => Err(e),
            };
            Msg::QueryDone { tab, res }
        });
    }

    pub fn handle(&mut self, msg: Msg) {
        let Msg::QueryDone { res, .. } = msg else { return };
        let elapsed = self.running.take().map(|t| t.elapsed()).unwrap_or_default();
        match res {
            Ok(sets) => {
                let mut views = Vec::new();
                for rs in sets {
                    if rs.columns.is_empty() {
                        let n = rs.affected.map(|a| format!("{a} righe interessate")).unwrap_or_else(|| "OK".into());
                        self.messages.push((n.to_string(), false));
                    } else {
                        let truncated = rs.rows.len() >= 500_000;
                        self.messages.push((format!("{} righe restituite{}", rs.rows.len(), if truncated { " (troncate)" } else { "" }), false));
                        let columns = grid_columns(&rs, None);
                        let i = views.len();
                        views.push(ResultView { rs, columns, grid: DataGrid::new(("result", self.id, i, self.messages.len())) });
                    }
                }
                self.messages.push((format!("Tempo: {:.3}s", elapsed.as_secs_f64()), false));
                self.active_result = if views.is_empty() { 0 } else { 1 };
                self.last = format!("{} risultati — {:.3}s", views.len(), elapsed.as_secs_f64());
                self.results = views;
            }
            Err(e) => {
                self.messages.push((e.to_string(), true));
                self.messages.push((format!("Tempo: {:.3}s", elapsed.as_secs_f64()), false));
                self.active_result = 0;
                self.last = "Errore".into();
            }
        }
    }

    pub fn status_text(&self) -> String {
        match self.running {
            Some(t) => format!("In esecuzione… {:.1}s", t.elapsed().as_secs_f64()),
            None => self.last.clone(),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        let mut run: Option<String> = None;
        if std::mem::take(&mut self.pending_run) {
            run = Some(self.sql.clone());
        }
        ui.horizontal(|ui| {
            if ui.add_enabled(self.running.is_none(), egui::Button::new(RichText::new("▶ Esegui").strong())).on_hover_text("⌘↩ / Ctrl+Invio").clicked() {
                run = Some(self.sql.clone());
            }
            if ui.add_enabled(self.running.is_none(), egui::Button::new("▶ Esegui selezione")).clicked() {
                run = Some(String::from("\u{0}selection"));
            }
            ui.separator();
            if ui.button("📂 Apri").clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("SQL", &["sql"]).pick_file()
            {
                match std::fs::read_to_string(&p) {
                    Ok(t) => {
                        self.sql = t;
                        self.path = Some(p);
                    }
                    Err(e) => env.actions.push(Action::Notice(e.to_string(), true)),
                }
            }
            if ui.button("💾 Salva").clicked() {
                let path = self.path.clone().or_else(|| rfd::FileDialog::new().add_filter("SQL", &["sql"]).set_file_name("query.sql").save_file());
                if let Some(p) = path {
                    match std::fs::write(&p, &self.sql) {
                        Ok(()) => {
                            if let Some(name) = p.file_name() {
                                env.actions.push(Action::SetTitle(self.id, name.to_string_lossy().into_owned()));
                            }
                            self.path = Some(p);
                        }
                        Err(e) => env.actions.push(Action::Notice(e.to_string(), true)),
                    }
                }
            }
            if self.running.is_some() {
                ui.spinner();
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        });

        let editor_id = egui::Id::unique(("sql-editor", self.id));
        egui::Panel::top(egui::Id::unique(("qedit", self.id))).resizable(true).default_size(260.0).min_size(80.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(("qscroll", self.id)).show(ui, |ui| {
                let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
                let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, buf.as_str(), "sql");
                    job.wrap.max_width = wrap;
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let out = egui::TextEdit::multiline(&mut self.sql)
                    .id(editor_id)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(12)
                    .hint_text("Scrivi qui le istruzioni SQL…")
                    .layouter(&mut layouter)
                    .show(ui);
                if out.response.has_focus() && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
                    run = Some(self.sql.clone());
                }
                let selection: Option<String> = out.cursor_range.and_then(|r| {
                    let range = r.as_sorted_char_range();
                    (range.start.0 != range.end.0).then(|| self.sql.chars().skip(range.start.0).take(range.end.0 - range.start.0).collect())
                });
                if let Some(s) = &selection {
                    ui.ctx().data_mut(|d| d.insert_temp(editor_id.with("sel"), s.clone()));
                } else if out.response.changed() || out.response.clicked() {
                    ui.ctx().data_mut(|d| d.remove::<String>(editor_id.with("sel")));
                }
            });
        });

        if let Some(sql) = run {
            let sql = if sql == "\u{0}selection" {
                ui.ctx().data(|d| d.get_temp::<String>(editor_id.with("sel"))).unwrap_or_else(|| self.sql.clone())
            } else {
                sql
            };
            self.run(env, sql);
        }

        // Result tabs.
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_result, 0, "Messaggio");
            for i in 0..self.results.len() {
                ui.selectable_value(&mut self.active_result, i + 1, format!("Risultato {}", i + 1));
            }
            if let Some(v) = self.active_result.checked_sub(1).and_then(|i| self.results.get(i)) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("📤 Esporta risultato").clicked() {
                        env.actions.push(Action::Export(ExportSource::Rows(Box::new(v.rs.clone()), "risultato".into(), self.loc.conn.clone())));
                    }
                    ui.label(RichText::new(format!("{} righe", v.rs.rows.len())).small());
                });
            }
        });
        ui.separator();
        match self.active_result.checked_sub(1).and_then(|i| self.results.get_mut(i)) {
            Some(v) => {
                let model = ResultModel { rs: &v.rs, columns: &v.columns };
                let _ = v.grid.show(ui, &model);
            }
            None => {
                egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    for (m, err) in &self.messages {
                        let t = RichText::new(m).monospace();
                        ui.label(if *err { t.color(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C)) } else { t });
                    }
                });
            }
        }
    }
}
