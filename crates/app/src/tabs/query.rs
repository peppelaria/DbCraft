//! Query editor: SQL with syntax highlighting, run all / run selection, one result tab per
//! statement plus a message log.

use crate::icons::{self, Db, G};
use std::time::Instant;

use db_core::{Dialect, ResultSet};
use dbgrid::{DataGrid, GridColumn};
use egui::RichText;

use crate::app::{Action, Env, Location};
use crate::backend::{Msg, TabId};
use crate::dialogs::ExportSource;
use crate::models::{ResultModel, grid_columns};
use crate::tabs::builder::{BuilderRequest, QueryBuilder};

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
    /// The running script changes the schema: refresh the tree when it finishes.
    ddl: bool,
    /// The first result belongs to the `SET search_path` prefix.
    skip_first: bool,
    builder: Option<QueryBuilder>,
    show_builder: bool,
}

fn changes_schema(sql: &str) -> bool {
    sql.split(';').any(|s| {
        let w = s.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        matches!(w.as_str(), "CREATE" | "DROP" | "ALTER" | "RENAME" | "TRUNCATE" | "COMMENT")
    })
}

impl QueryTab {
    pub fn new(id: TabId, loc: Location) -> Self {
        QueryTab { id, loc, sql: String::new(), results: Vec::new(), active_result: 0, messages: Vec::new(), running: None, last: String::new(), path: None, pending_run: false, ddl: false, skip_first: false, builder: None, show_builder: false }
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
        self.ddl = changes_schema(&sql);
        // PostgreSQL: unqualified names resolve in the schema selected in the tree.
        let sql = match (&self.loc.schema, cfg.kind) {
            (Some(s), db_core::DbKind::Postgres) if s != "public" => {
                self.skip_first = true;
                format!("SET search_path TO {}, public;\n{sql}", db_core::Dialect::Postgres.quote(s))
            }
            _ => {
                self.skip_first = false;
                sql
            }
        };
        let tab = self.id;
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.execute(&sql).await,
                Err(e) => Err(e),
            };
            Msg::QueryDone { tab, res }
        });
    }

    pub fn handle(&mut self, msg: Msg, env: &mut Env<'_>) {
        let res = match msg {
            Msg::QueryDone { res, .. } => res,
            Msg::BuilderTables { res, .. } => {
                if let Some(b) = self.builder.as_mut() {
                    b.on_tables(res);
                }
                return;
            }
            Msg::BuilderTable { name, res, .. } => {
                if let Some(b) = self.builder.as_mut() {
                    b.on_table_info(&name, res);
                }
                return;
            }
            _ => return,
        };
        if std::mem::take(&mut self.ddl) {
            env.actions.push(Action::Refresh(self.loc.clone()));
        }
        let skip = usize::from(std::mem::take(&mut self.skip_first));
        let elapsed = self.running.take().map(|t| t.elapsed()).unwrap_or_default();
        match res {
            Ok(sets) => {
                let mut views = Vec::new();
                for rs in sets.into_iter().skip(skip) {
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

    pub fn open_builder(&mut self, env: &mut Env<'_>) {
        self.show_builder = true;
        if self.builder.is_some() {
            return;
        }
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        self.builder = Some(QueryBuilder::new(Dialect::from(cfg.kind), self.loc.schema.clone()));
        let (tab, schema) = (self.id, self.loc.schema.clone());
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.list_objects(schema.as_deref()).await,
                Err(e) => Err(e),
            };
            Msg::BuilderTables { tab, res }
        });
    }

    /// Scripted builder state for screenshots (`--builder tables;fields`).
    pub fn debug_builder(&mut self, env: &mut Env<'_>, spec: &str) {
        self.open_builder(env);
        let (tables, fields) = spec.split_once(';').unwrap_or((spec, ""));
        if let Some(b) = self.builder.as_mut() {
            for t in tables.split(',').filter(|t| !t.is_empty()) {
                b.add_table(t, None);
            }
            for f in fields.split(',').filter(|f| !f.is_empty()) {
                b.debug_field(f);
            }
        }
        self.builder_requests(env);
    }

    /// Sends the table definitions the builder asked for.
    fn builder_requests(&mut self, env: &mut Env<'_>) {
        let Some(b) = self.builder.as_mut() else { return };
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        for BuilderRequest::Describe(name) in std::mem::take(&mut b.requests) {
            let (tab, schema) = (self.id, self.loc.schema.clone());
            env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
                let res = match d {
                    Ok(d) => d.describe_table(schema.as_deref(), &name).await,
                    Err(e) => Err(e),
                };
                Msg::BuilderTable { tab, name, res }
            });
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        if self.show_builder {
            let done = self.builder.as_mut().and_then(|b| b.ui(ui));
            self.builder_requests(env);
            match done {
                Some(true) => {
                    if let Some(b) = &self.builder {
                        self.sql = b.sql();
                    }
                    self.show_builder = false;
                }
                Some(false) => self.show_builder = false,
                None => {}
            }
            return;
        }
        let mut run: Option<String> = None;
        if std::mem::take(&mut self.pending_run) {
            run = Some(self.sql.clone());
        }
        ui.horizontal(|ui| {
            if icons::button_ex(ui, Db::Run, "Esegui", false, self.running.is_none()).on_hover_text("⌘↩ / Ctrl+Invio").clicked() {
                run = Some(self.sql.clone());
            }
            if icons::button_ex(ui, Db::RunSelection, "Esegui selezione", false, self.running.is_none()).clicked() {
                run = Some(String::from("\u{0}selection"));
            }
            if icons::button(ui, Db::QueryBuilder, "Query Builder").on_hover_text("Costruisci la query visualmente").clicked() {
                self.open_builder(env);
            }
            ui.separator();
            if icons::button(ui, G::OpenFolder, "Apri").clicked()
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
            if icons::button(ui, G::Save, "Salva").clicked() {
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
                    if icons::button(ui, Db::Export, "Esporta risultato").clicked() {
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
