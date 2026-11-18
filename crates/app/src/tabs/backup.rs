//! The "Backup" tab (as in Navicat): the backups of a database/schema, kept as SQL scripts in
//! DbCraft's config folder, with "Nuovo backup", "Ripristina", "Elimina" and "Mostra cartella".

use egui::RichText;

use crate::app::{Action, Env, Location};
use crate::backend::{Msg, TabId};
use crate::icons::{self, Db, G};
use crate::store::{self, FileEntry};
use crate::i18n;

enum Ask {
    Restore(FileEntry),
    Delete(FileEntry),
}

pub struct BackupTab {
    pub id: TabId,
    pub loc: Location,
    files: Vec<FileEntry>,
    selected: Option<usize>,
    /// Progress of the running backup / restore.
    running: Option<String>,
    ask: Option<Ask>,
    last: Option<Result<String, String>>,
}

pub fn fmt_size(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}

impl BackupTab {
    pub fn new(id: TabId, loc: Location) -> Self {
        let mut t = BackupTab { id, loc, files: Vec::new(), selected: None, running: None, ask: None, last: None };
        t.refresh();
        t
    }

    fn dir(&self) -> std::path::PathBuf {
        store::place_dir("backups", &self.loc.conn, &self.loc.db, self.loc.schema.as_deref())
    }

    fn refresh(&mut self) {
        self.files = store::list_files(&self.dir(), "sql");
        self.selected = self.selected.filter(|&i| i < self.files.len());
    }

    pub fn handle(&mut self, msg: Msg, env: &mut Env<'_>) {
        match msg {
            Msg::DumpProgress { text, .. } => self.running = Some(text),
            Msg::Dumped { res, .. } => {
                self.running = None;
                match &res {
                    Ok(t) => env.actions.push(Action::Notice(t.clone(), false)),
                    Err(e) => env.actions.push(Action::Notice(format!("Backup/ripristino non riuscito: {e}"), true)),
                }
                if res.is_ok() {
                    env.actions.push(Action::Refresh(self.loc.clone()));
                }
                self.last = Some(res);
                self.refresh();
            }
            _ => {}
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        let busy = self.running.is_some();
        let sel = self.selected.and_then(|i| self.files.get(i)).cloned();
        ui.horizontal_wrapped(|ui| {
            if icons::button_ex(ui, Db::Backup, "Nuovo backup", false, !busy).clicked()
                && let Some(cfg) = env.cfg(&self.loc.conn).cloned()
            {
                self.running = Some("Inizio il backup…".into());
                crate::dump::start_backup(env.backend, &cfg, &self.loc, Some(self.id), None);
            }
            if icons::button_ex(ui, Db::Import, "Ripristina backup", false, !busy && sel.is_some()).clicked()
                && let Some(f) = sel.clone()
            {
                self.ask = Some(Ask::Restore(f));
            }
            if icons::button_ex(ui, G::Delete, "Elimina backup", false, !busy && sel.is_some()).clicked()
                && let Some(f) = sel.clone()
            {
                self.ask = Some(Ask::Delete(f));
            }
            ui.separator();
            if icons::button_ex(ui, G::OpenFolder, "Ripristina da file…", false, !busy).clicked()
                && let Some(p) = rfd::FileDialog::new().add_filter("SQL", &["sql"]).pick_file()
                && let Ok(md) = std::fs::metadata(&p)
            {
                let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                self.ask = Some(Ask::Restore(FileEntry { path: p, name, modified: String::new(), size: md.len() }));
            }
            if icons::button(ui, G::Folder, "Mostra cartella").clicked() {
                let dir = self.dir();
                let _ = std::fs::create_dir_all(&dir);
                crate::selfupdate::open_url(&dir.to_string_lossy());
            }
            if icons::button(ui, Db::Refresh, "Aggiorna").clicked() {
                self.refresh();
            }
        });
        ui.separator();
        if let Some(p) = &self.running {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(i18n::tr(p).into_owned());
            });
            ui.separator();
        } else if let Some(Err(e)) = &self.last {
            ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
            ui.separator();
        }
        let what = match &self.loc.schema {
            Some(s) => format!("schema {s} del database {}", self.loc.db),
            None if self.loc.db.is_empty() => "database predefinito".to_string(),
            None => format!("database {}", self.loc.db),
        };
        ui.label(RichText::new(i18n::tf(format!("Backup del {what}: struttura e dati di tutte le tabelle, più le viste, in uno script SQL."))).weak());
        ui.add_space(4.0);
        let mut open = None;
        if self.files.is_empty() {
            ui.label(RichText::new(i18n::t("Nessun backup. Usa «Nuovo backup».")).weak());
        }
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .auto_shrink([false, false])
            .column(egui_extras::Column::initial(260.0).at_least(120.0))
            .column(egui_extras::Column::initial(160.0))
            .column(egui_extras::Column::remainder())
            .header(22.0, |mut h| {
                for t in ["Backup", "Data", "Dimensione"] {
                    h.col(|ui| {
                        ui.strong(i18n::t(t));
                    });
                }
            })
            .body(|body| {
                body.rows(22.0, self.files.len(), |mut row| {
                    let i = row.index();
                    let Some(f) = self.files.get(i) else { return };
                    row.set_selected(self.selected == Some(i));
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            icons::show(ui, Db::Backup, 16.0);
                            ui.label(f.name.as_str());
                        });
                    });
                    row.col(|ui| {
                        ui.label(f.modified.as_str());
                    });
                    row.col(|ui| {
                        ui.label(fmt_size(f.size));
                    });
                    let r = row.response();
                    if r.clicked() {
                        self.selected = Some(i);
                    }
                    if r.double_clicked() {
                        open = Some(f.clone());
                    }
                });
            });
        if let Some(f) = open {
            self.ask = Some(Ask::Restore(f));
        }
        self.ask_ui(ui.ctx(), env);
    }

    fn ask_ui(&mut self, ctx: &egui::Context, env: &mut Env<'_>) {
        let Some(ask) = &self.ask else { return };
        let (text, verb) = match ask {
            Ask::Restore(f) => (
                format!(
                    "Ripristinare il backup \"{}\" sul database {}?\n\nLe tabelle e le viste del backup vengono ricreate: i dati attuali di quelle tabelle vanno persi.",
                    f.name,
                    if self.loc.db.is_empty() { "predefinito" } else { &self.loc.db }
                ),
                "Ripristina",
            ),
            Ask::Delete(f) => (format!("Eliminare il backup \"{}\"?", f.name), "Elimina"),
        };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique(("backup_ask", self.id))).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(i18n::tf(text));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(i18n::t(verb)).clicked() {
                    answer = Some(true);
                }
                if ui.button(i18n::t("Annulla")).clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => match self.ask.take() {
                Some(Ask::Restore(f)) => {
                    if let Some(cfg) = env.cfg(&self.loc.conn).cloned() {
                        self.running = Some("Inizio il ripristino…".into());
                        crate::dump::start_restore(env.backend, &cfg, &self.loc, self.id, f.path);
                    }
                }
                Some(Ask::Delete(f)) => {
                    if let Err(e) = std::fs::remove_file(&f.path) {
                        env.actions.push(Action::Notice(e.to_string(), true));
                    }
                    self.refresh();
                }
                None => {}
            },
            Some(false) => self.ask = None,
            None => {}
        }
    }
}
