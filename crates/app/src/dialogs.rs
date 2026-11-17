//! Modal windows: connection editor, export and import wizards, confirmations.

use std::path::PathBuf;

use db_core::{ConnectionConfig, DbError, DbKind, DbResult, Dialect, ResultSet, RowChange, TableInfo, Value};
use egui::RichText;

use crate::app::{ConfirmKind, ObjRef};
use crate::backend::{Backend, Msg};
use crate::export::{self, CsvData, Format};

fn modal<R>(ctx: &egui::Context, id: &str, title: &str, body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let modal = egui::Modal::new(egui::Id::unique(id)).show(ctx, |ui| {
        ui.set_min_width(460.0);
        ui.heading(title);
        ui.add_space(6.0);
        body(ui)
    });
    Some(modal.inner)
}

// ----- connection -----

pub enum ConnOutcome {
    Save(ConnectionConfig),
    Cancel,
}

pub struct ConnDialog {
    cfg: ConnectionConfig,
    is_new: bool,
    port: String,
}

impl ConnDialog {
    pub fn new(kind: DbKind) -> Self {
        let cfg = ConnectionConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("{} locale", kind.label()),
            kind,
            host: if kind == DbKind::Sqlite { String::new() } else { "localhost".into() },
            port: kind.default_port(),
            user: kind.default_user().into(),
            save_password: true,
            ..Default::default()
        };
        ConnDialog { port: cfg.port.to_string(), cfg, is_new: true }
    }

    pub fn edit(cfg: ConnectionConfig) -> Self {
        ConnDialog { port: cfg.port.to_string(), cfg, is_new: false }
    }

    pub fn show(&mut self, ctx: &egui::Context, backend: &Backend) -> Option<ConnOutcome> {
        let title = format!("{} — {}", if self.is_new { "Nuova connessione" } else { "Modifica connessione" }, self.cfg.kind.label());
        modal(ctx, "conn-dialog", &title, |ui| {
            let mut out = None;
            let sqlite = self.cfg.kind == DbKind::Sqlite;
            egui::Grid::new("conn-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Nome connessione:");
                ui.add(egui::TextEdit::singleline(&mut self.cfg.name).desired_width(280.0));
                ui.end_row();
                if sqlite {
                    ui.label("File database:");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.cfg.database).desired_width(220.0).hint_text(":memory:"));
                        if ui.button("…").on_hover_text("Scegli o crea un file").clicked()
                            && let Some(p) = rfd::FileDialog::new().add_filter("SQLite", &["db", "sqlite", "sqlite3", "db3"]).set_file_name("database.db").save_file()
                        {
                            self.cfg.database = p.to_string_lossy().into_owned();
                        }
                        if ui.button("Apri…").clicked()
                            && let Some(p) = rfd::FileDialog::new().pick_file()
                        {
                            self.cfg.database = p.to_string_lossy().into_owned();
                        }
                    });
                    ui.end_row();
                } else {
                    ui.label("Host:");
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.host).desired_width(280.0));
                    ui.end_row();
                    ui.label("Porta:");
                    if ui.add(egui::TextEdit::singleline(&mut self.port).desired_width(80.0)).changed() {
                        self.cfg.port = self.port.trim().parse().unwrap_or(self.cfg.port);
                    }
                    ui.end_row();
                    ui.label("Nome utente:");
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.user).desired_width(280.0));
                    ui.end_row();
                    ui.label("Password:");
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.password).password(true).desired_width(280.0));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut self.cfg.save_password, "Salva password (Portachiavi)");
                    ui.end_row();
                    ui.label("Database iniziale:");
                    ui.add(egui::TextEdit::singleline(&mut self.cfg.database).desired_width(280.0).hint_text("(facoltativo)"));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut self.cfg.ssl, "Richiedi SSL/TLS");
                    ui.end_row();
                }
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Prova connessione").clicked() {
                    let cfg = self.cfg.clone();
                    backend.spawn(async move {
                        match db_drivers::connect(&cfg, None).await {
                            Ok(d) => {
                                let r = d.list_databases().await;
                                d.close().await;
                                match r {
                                    Ok(v) => Msg::Notice { text: format!("Connessione riuscita ({} database)", v.len()), error: false },
                                    Err(e) => Msg::Notice { text: e.to_string(), error: true },
                                }
                            }
                            Err(e) => Msg::Notice { text: e.to_string(), error: true },
                        }
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!self.cfg.name.trim().is_empty(), egui::Button::new(RichText::new("OK").strong())).clicked() {
                        out = Some(ConnOutcome::Save(self.cfg.clone()));
                    }
                    if ui.button("Annulla").clicked() {
                        out = Some(ConnOutcome::Cancel);
                    }
                });
            });
            out
        })
        .flatten()
    }
}

// ----- export -----

pub enum ExportSource {
    Table(ObjRef),
    /// Rows already loaded, a base name and the connection (for the SQL dialect).
    Rows(Box<ResultSet>, String, String),
}

pub struct ExportDialog {
    source: ExportSource,
    format: Format,
    header: bool,
}

impl ExportDialog {
    pub fn new(source: ExportSource) -> Self {
        ExportDialog { source, format: Format::Csv, header: true }
    }

    /// Returns false when the dialog should close.
    pub fn show(&mut self, ctx: &egui::Context, backend: &Backend, configs: &[ConnectionConfig]) -> bool {
        let name = match &self.source {
            ExportSource::Table(o) => o.name.clone(),
            ExportSource::Rows(_, n, _) => n.clone(),
        };
        let conn = match &self.source {
            ExportSource::Table(o) => o.conn.clone(),
            ExportSource::Rows(_, _, c) => c.clone(),
        };
        let dialect = configs.iter().find(|c| c.id == conn).map_or(Dialect::Sqlite, |c| Dialect::from(c.kind));
        modal(ctx, "export-dialog", &format!("Esporta «{name}»"), |ui| {
            for f in Format::ALL {
                ui.radio_value(&mut self.format, f, f.label());
            }
            ui.add_space(4.0);
            if self.format != Format::Json && self.format != Format::Sql {
                ui.checkbox(&mut self.header, "Includi intestazioni di colonna");
            }
            ui.add_space(8.0);
            let mut keep = true;
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Esporta…").strong()).clicked() {
                        let fmt = self.format;
                        if let Some(path) = rfd::FileDialog::new().add_filter(fmt.ext(), &[fmt.ext()]).set_file_name(format!("{name}.{}", fmt.ext())).save_file() {
                            let header = self.header;
                            start_export(backend, configs, &self.source, path, fmt, header, dialect, name.clone());
                            keep = false;
                        }
                    }
                    if ui.button("Annulla").clicked() {
                        keep = false;
                    }
                });
            });
            keep
        })
        .unwrap_or(false)
    }
}

#[allow(clippy::too_many_arguments)]
fn start_export(backend: &Backend, configs: &[ConnectionConfig], source: &ExportSource, path: PathBuf, fmt: Format, header: bool, dialect: Dialect, name: String) {
    let write = move |rs: ResultSet| async move {
        let p = path.clone();
        let res = tokio::task::spawn_blocking(move || export::export(&rs, &p, fmt, dialect, &name, header)).await;
        match res {
            Ok(Ok(n)) => Msg::Notice { text: format!("Esportati {n} record in {}", path.display()), error: false },
            Ok(Err(e)) => Msg::Notice { text: format!("Esportazione fallita: {e}"), error: true },
            Err(e) => Msg::Notice { text: format!("Esportazione fallita: {e}"), error: true },
        }
    };
    match source {
        ExportSource::Rows(rs, _, _) => {
            let rs = (**rs).clone();
            backend.spawn(write(rs));
        }
        ExportSource::Table(obj) => {
            let Some(cfg) = configs.iter().find(|c| c.id == obj.conn) else { return };
            let sql = format!("SELECT * FROM {}", dialect.qualified(obj.schema.as_deref(), &obj.name));
            backend.with_driver(cfg, &obj.db, move |d| async move {
                let rs = match d {
                    Ok(d) => d.execute(&sql).await,
                    Err(e) => Err(e),
                };
                match rs.map(|v| v.into_iter().next().unwrap_or_default()) {
                    Ok(rs) => write(rs).await,
                    Err(e) => Msg::Notice { text: format!("Esportazione fallita: {e}"), error: true },
                }
            });
        }
    }
}

// ----- import -----

pub struct ImportDialog {
    target: ObjRef,
    path: Option<PathBuf>,
    delimiter: String,
    header: bool,
    empty_null: bool,
    data: Option<CsvData>,
    columns: Vec<String>,
    /// For each CSV column, the target column index.
    mapping: Vec<Option<usize>>,
    pub running: bool,
    error: Option<String>,
}

impl ImportDialog {
    pub fn new(target: ObjRef) -> Self {
        ImportDialog {
            target,
            path: None,
            delimiter: ",".into(),
            header: true,
            empty_null: true,
            data: None,
            columns: Vec::new(),
            mapping: Vec::new(),
            running: true,
            error: None,
        }
    }

    pub fn on_info(&mut self, res: DbResult<TableInfo>) {
        self.running = false;
        match res {
            Ok(info) => {
                self.columns = info.columns.into_iter().map(|c| c.name).collect();
                self.automap();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn automap(&mut self) {
        let Some(d) = &self.data else { return };
        self.mapping = d
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| self.columns.iter().position(|c| c.eq_ignore_ascii_case(h.trim())).or((!self.header && i < self.columns.len()).then_some(i)))
            .collect();
    }

    fn read(&mut self) {
        let Some(p) = &self.path else { return };
        let delim = match self.delimiter.as_str() {
            "\\t" | "TAB" | "tab" => b'\t',
            s => s.bytes().next().unwrap_or(b','),
        };
        match export::read_csv(p, delim, self.header) {
            Ok(d) => {
                self.data = Some(d);
                self.error = None;
                self.automap();
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, backend: &Backend, configs: &[ConnectionConfig]) -> bool {
        let title = format!("Importa in «{}»", self.target.name);
        modal(ctx, "import-dialog", &title, |ui| {
            ui.set_max_width(720.0);
            let mut keep = true;
            ui.horizontal(|ui| {
                if ui.button("📂 Scegli file CSV…").clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("CSV", &["csv", "txt", "tsv"]).pick_file()
                {
                    self.path = Some(p);
                    self.read();
                }
                if let Some(p) = &self.path {
                    ui.label(RichText::new(p.display().to_string()).small());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Delimitatore:");
                let mut changed = ui.add(egui::TextEdit::singleline(&mut self.delimiter).desired_width(30.0)).changed();
                changed |= ui.checkbox(&mut self.header, "Prima riga = intestazioni").changed();
                ui.checkbox(&mut self.empty_null, "Celle vuote = NULL");
                if changed {
                    self.read();
                }
            });
            if let Some(e) = &self.error {
                ui.colored_label(egui::Color32::RED, e);
            }
            if let Some(d) = &self.data {
                ui.separator();
                ui.label(format!("{} righe lette. Associa i campi:", d.rows.len()));
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    egui::Grid::new("import-map").num_columns(3).striped(true).show(ui, |ui| {
                        ui.strong("Campo sorgente");
                        ui.strong("Esempio");
                        ui.strong("Campo destinazione");
                        ui.end_row();
                        for (i, h) in d.headers.iter().enumerate() {
                            ui.label(h);
                            ui.label(RichText::new(d.rows.first().and_then(|r| r.get(i)).cloned().unwrap_or_default()).weak());
                            let cur = self.mapping.get(i).copied().flatten();
                            let text = cur.and_then(|c| self.columns.get(c)).cloned().unwrap_or_else(|| "(ignora)".into());
                            egui::ComboBox::from_id_salt(("map", i)).selected_text(text).show_ui(ui, |ui| {
                                if self.mapping.len() <= i {
                                    self.mapping.resize(i + 1, None);
                                }
                                ui.selectable_value(&mut self.mapping[i], None, "(ignora)");
                                for (k, c) in self.columns.iter().enumerate() {
                                    ui.selectable_value(&mut self.mapping[i], Some(k), c);
                                }
                            });
                            ui.end_row();
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if self.running {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = self.data.is_some() && self.mapping.iter().any(Option::is_some) && !self.running;
                    if ui.add_enabled(ready, egui::Button::new(RichText::new("Importa").strong())).clicked() {
                        self.start(backend, configs);
                    }
                    if ui.button("Chiudi").clicked() {
                        keep = false;
                    }
                });
            });
            keep
        })
        .unwrap_or(false)
    }

    fn start(&mut self, backend: &Backend, configs: &[ConnectionConfig]) {
        let (Some(d), Some(cfg)) = (&self.data, configs.iter().find(|c| c.id == self.target.conn)) else { return };
        let changes: Vec<RowChange> = d
            .rows
            .iter()
            .map(|row| RowChange::Insert {
                values: self
                    .mapping
                    .iter()
                    .enumerate()
                    .filter_map(|(i, m)| {
                        let col = self.columns.get((*m)?)?;
                        let raw = row.get(i).cloned().unwrap_or_default();
                        let v = if raw.is_empty() && self.empty_null { Value::Null } else { Value::Text(raw) };
                        Some((col.clone(), v))
                    })
                    .collect(),
            })
            .collect();
        self.running = true;
        let obj = self.target.clone();
        backend.with_driver(cfg, &obj.db, move |drv| async move {
            let res: DbResult<u64> = async {
                let drv = drv?;
                let mut total = 0;
                for chunk in changes.chunks(500) {
                    total += drv.apply_changes(obj.schema.as_deref(), &obj.name, chunk).await.map_err(|e| DbError::Query(format!("dopo {total} righe: {e}")))?;
                }
                Ok(total)
            }
            .await;
            Msg::Imported { res }
        });
    }
}

// ----- confirm -----

pub struct ConfirmDialog {
    text: String,
    pub kind: ConfirmKind,
}

impl ConfirmDialog {
    pub fn new(text: String, kind: ConfirmKind) -> Self {
        ConfirmDialog { text, kind }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<bool> {
        modal(ctx, "confirm-dialog", "Conferma", |ui| {
            ui.label(&self.text);
            ui.add_space(10.0);
            let mut out = None;
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("Elimina").color(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C)).strong()).clicked() {
                    out = Some(true);
                }
                if ui.button("Annulla").clicked() {
                    out = Some(false);
                }
            });
            out
        })
        .flatten()
    }
}
