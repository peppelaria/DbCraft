//! The "Automazione" tab: scheduled jobs (a backup, or some SQL) that run by themselves at the
//! chosen times while DbCraft is open. The jobs live in `automation.json`; the App runs them
//! (see `App::run_due_jobs`).

use std::collections::HashSet;

use db_core::ConnectionConfig;
use egui::RichText;

use crate::app::{Action, Env, Location};
use crate::backend::TabId;
use crate::icons::{self, Db, G};
use crate::store::{self, Job, JobAction, Schedule};
use crate::i18n;

pub struct AutomationTab {
    pub id: TabId,
    selected: Option<u64>,
    /// The job being edited (new or existing).
    editing: Option<Job>,
    confirm_delete: bool,
}

const DAYS: [&str; 7] = ["Lunedì", "Martedì", "Mercoledì", "Giovedì", "Venerdì", "Sabato", "Domenica"];

impl AutomationTab {
    pub fn new(id: TabId) -> Self {
        AutomationTab { id, selected: None, editing: None, confirm_delete: false }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>, jobs: &mut Vec<Job>, running: &HashSet<u64>, here: Option<&Location>) {
        let sel = self.selected.and_then(|id| jobs.iter().position(|j| j.id == id));
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            if icons::button_ex(ui, Db::Automation, "Nuova attività", false, !env.configs.is_empty()).clicked() {
                let loc = here.cloned().or_else(|| env.configs.first().map(|c| Location { conn: c.id.clone(), db: c.database.clone(), schema: None })).unwrap_or_default();
                let schedule = Schedule::Daily { hour: 2, minute: 0 };
                self.editing = Some(Job {
                    id: jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1,
                    name: format!("Backup {}", if loc.db.is_empty() { env.cfg(&loc.conn).map(|c| c.name.clone()).unwrap_or_default() } else { loc.db.clone() }),
                    enabled: true,
                    conn: loc.conn,
                    db: loc.db,
                    schema: loc.schema,
                    action: JobAction::Backup,
                    next_run: schedule.next_after(store::now()),
                    schedule,
                    last_run: None,
                    last_result: None,
                });
            }
            if icons::button_ex(ui, Db::Design, "Modifica", false, sel.is_some()).clicked()
                && let Some(i) = sel
            {
                self.editing = jobs.get(i).cloned();
            }
            if icons::button_ex(ui, G::Delete, "Elimina", false, sel.is_some()).clicked() {
                self.confirm_delete = true;
            }
            ui.separator();
            let can_run = sel.is_some_and(|i| jobs.get(i).is_some_and(|j| !running.contains(&j.id)));
            if icons::button_ex(ui, Db::Run, "Esegui ora", false, can_run).clicked()
                && let Some(j) = sel.and_then(|i| jobs.get(i))
            {
                env.actions.push(Action::RunJob(j.id));
            }
            if let Some(j) = sel.and_then(|i| jobs.get_mut(i)) {
                let label = if j.enabled { "Disattiva" } else { "Attiva" };
                if icons::button(ui, if j.enabled { G::StopSquare } else { G::Play }, label).clicked() {
                    j.enabled = !j.enabled;
                    if j.enabled {
                        j.next_run = j.schedule.next_after(store::now());
                    }
                    changed = true;
                }
            }
        });
        ui.separator();
        ui.label(RichText::new(i18n::t("Le attività partono da sole all'orario scelto mentre DbCraft è aperto; i backup vanno nella scheda Backup del database.")).weak());
        ui.add_space(4.0);
        let conn_name = |id: &str| env.configs.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_else(|| "(connessione eliminata)".into());
        let mut edit = None;
        if jobs.is_empty() {
            ui.label(RichText::new(i18n::t("Nessuna attività. Usa «Nuova attività» per pianificare un backup o una query.")).weak());
        }
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .auto_shrink([false, false])
            .column(egui_extras::Column::initial(200.0).at_least(100.0))
            .column(egui_extras::Column::initial(90.0))
            .column(egui_extras::Column::initial(180.0))
            .column(egui_extras::Column::initial(180.0))
            .column(egui_extras::Column::initial(130.0))
            .column(egui_extras::Column::initial(130.0))
            .column(egui_extras::Column::remainder())
            .header(22.0, |mut h| {
                for t in ["Nome", "Azione", "Database", "Pianificazione", "Prossima", "Ultima", "Esito"] {
                    h.col(|ui| {
                        // one line: a narrow last column would otherwise stack its title letter by letter
                        ui.add(egui::Label::new(RichText::new(i18n::t(t)).strong()).truncate());
                    });
                }
            })
            .body(|body| {
                body.rows(22.0, jobs.len(), |mut row| {
                    let Some(j) = jobs.get(row.index()) else {
                        return;
                    };
                    row.set_selected(self.selected == Some(j.id));
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            icons::show(ui, Db::Automation, 16.0);
                            let t = RichText::new(j.name.as_str());
                            ui.label(if j.enabled { t } else { t.weak() });
                        });
                    });
                    row.col(|ui| {
                        ui.label(match j.action {
                            JobAction::Backup => i18n::t("Backup"),
                            JobAction::Query { .. } => i18n::t("Query"),
                        });
                    });
                    row.col(|ui| {
                        let mut s = conn_name(&j.conn);
                        if !j.db.is_empty() {
                            s = format!("{s} › {}", j.db);
                        }
                        if let Some(sc) = &j.schema {
                            s = format!("{s} › {sc}");
                        }
                        ui.add(egui::Label::new(s).truncate());
                    });
                    row.col(|ui| {
                        ui.label(i18n::tf(j.schedule.describe()));
                    });
                    row.col(|ui| {
                        if running.contains(&j.id) {
                            ui.label(i18n::t("in corso…"));
                        } else if j.enabled {
                            ui.label(store::fmt_time(j.next_run));
                        } else {
                            ui.label(RichText::new(i18n::t("disattivata")).weak());
                        }
                    });
                    row.col(|ui| {
                        ui.label(j.last_run.map(store::fmt_time).unwrap_or_default());
                    });
                    row.col(|ui| {
                        if let Some(r) = &j.last_result {
                            let err = r.starts_with("Errore");
                            let r = i18n::tr(r).into_owned();
                            let l = ui.add(egui::Label::new(if err { RichText::new(r.as_str()).color(ui.visuals().error_fg_color) } else { RichText::new(r.as_str()) }).truncate());
                            l.on_hover_text(r.as_str());
                        }
                    });
                    let r = row.response();
                    if r.clicked() {
                        self.selected = Some(j.id);
                    }
                    if r.double_clicked() {
                        edit = Some(j.clone());
                    }
                });
            });
        if edit.is_some() {
            self.editing = edit;
        }
        if self.confirm_delete {
            let name = sel.and_then(|i| jobs.get(i)).map(|j| j.name.clone()).unwrap_or_default();
            let mut answer = None;
            egui::Modal::new(egui::Id::unique(("job_delete", self.id))).show(ui.ctx(), |ui| {
                ui.label(i18n::tf(format!("Eliminare l'attività \"{name}\"?")));
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("Elimina")).clicked() {
                        answer = Some(true);
                    }
                    if ui.button(i18n::t("Annulla")).clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(yes) = answer {
                self.confirm_delete = false;
                if yes && let Some(i) = sel {
                    jobs.remove(i);
                    self.selected = None;
                    changed = true;
                }
            }
        }
        if let Some(saved) = self.editor_ui(ui.ctx(), env.configs) {
            match jobs.iter_mut().find(|j| j.id == saved.id) {
                Some(j) => *j = saved,
                None => {
                    self.selected = Some(saved.id);
                    jobs.push(saved);
                }
            }
            changed = true;
        }
        if changed && let Err(e) = store::save_jobs(jobs) {
            env.actions.push(Action::Notice(e, true));
        }
    }

    /// The job editor; returns the job when saved.
    fn editor_ui(&mut self, ctx: &egui::Context, configs: &[ConnectionConfig]) -> Option<Job> {
        let job = self.editing.as_mut()?;
        let mut save = false;
        let mut close = false;
        egui::Modal::new(egui::Id::unique(("job_edit", self.id))).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading(i18n::t("Attività pianificata"));
            ui.add_space(4.0);
            egui::Grid::new("job_grid").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label(i18n::t("Nome"));
                ui.add(egui::TextEdit::singleline(&mut job.name).desired_width(320.0));
                ui.end_row();
                ui.label(i18n::t("Connessione"));
                let current = configs.iter().find(|c| c.id == job.conn).map(|c| c.name.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt("job_conn").selected_text(current).width(320.0).show_ui(ui, |ui| {
                    for c in configs.iter().filter(|c| !c.id.starts_with(store::TRANSIENT)) {
                        if ui.selectable_label(c.id == job.conn, c.name.as_str()).clicked() {
                            job.conn = c.id.clone();
                            job.db = c.database.clone();
                        }
                    }
                });
                ui.end_row();
                ui.label(i18n::t("Database"));
                ui.add(egui::TextEdit::singleline(&mut job.db).hint_text(i18n::t("predefinito della connessione")).desired_width(320.0));
                ui.end_row();
                ui.label(i18n::t("Schema"));
                let mut schema = job.schema.clone().unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut schema).hint_text(i18n::t("PostgreSQL / SQL Server")).desired_width(320.0)).changed() {
                    job.schema = (!schema.trim().is_empty()).then(|| schema.trim().to_string());
                }
                ui.end_row();
                ui.label(i18n::t("Azione"));
                ui.horizontal(|ui| {
                    if ui.radio(job.action == JobAction::Backup, i18n::t("Backup")).clicked() {
                        job.action = JobAction::Backup;
                    }
                    if ui.radio(matches!(job.action, JobAction::Query { .. }), i18n::t("Esegui SQL")).clicked() && job.action == JobAction::Backup {
                        job.action = JobAction::Query { sql: String::new() };
                    }
                });
                ui.end_row();
                if let JobAction::Query { sql } = &mut job.action {
                    ui.label(i18n::t("SQL"));
                    ui.add(egui::TextEdit::multiline(sql).code_editor().desired_rows(5).desired_width(320.0).hint_text(i18n::t("Una o più istruzioni separate da ;")));
                    ui.end_row();
                }
                ui.label(i18n::t("Quando"));
                ui.vertical(|ui| {
                    let mut kind = match job.schedule {
                        Schedule::Every { .. } => 0,
                        Schedule::Daily { .. } => 1,
                        Schedule::Weekly { .. } => 2,
                    };
                    let before = kind;
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut kind, 0, i18n::t("A intervalli"));
                        ui.radio_value(&mut kind, 1, i18n::t("Ogni giorno"));
                        ui.radio_value(&mut kind, 2, i18n::t("Ogni settimana"));
                    });
                    if kind != before {
                        job.schedule = match kind {
                            0 => Schedule::Every { minutes: 60 },
                            1 => Schedule::Daily { hour: 2, minute: 0 },
                            _ => Schedule::Weekly { weekday: 0, hour: 2, minute: 0 },
                        };
                    }
                    ui.horizontal(|ui| match &mut job.schedule {
                        Schedule::Every { minutes } => {
                            ui.label(i18n::t("ogni"));
                            ui.add(egui::DragValue::new(minutes).range(1..=10_080));
                            ui.label(i18n::t("minuti"));
                        }
                        Schedule::Daily { hour, minute } => {
                            ui.label(i18n::t("alle"));
                            ui.add(egui::DragValue::new(hour).range(0..=23));
                            ui.label(":");
                            ui.add(egui::DragValue::new(minute).range(0..=59).custom_formatter(|v, _| format!("{v:02}")));
                        }
                        Schedule::Weekly { weekday, hour, minute } => {
                            egui::ComboBox::from_id_salt("job_day").selected_text(i18n::t(DAYS[(*weekday as usize).min(6)])).show_ui(ui, |ui| {
                                for (i, d) in DAYS.iter().enumerate() {
                                    ui.selectable_value(weekday, i as u32, i18n::t(d));
                                }
                            });
                            ui.label(i18n::t("alle"));
                            ui.add(egui::DragValue::new(hour).range(0..=23));
                            ui.label(":");
                            ui.add(egui::DragValue::new(minute).range(0..=59).custom_formatter(|v, _| format!("{v:02}")));
                        }
                    });
                });
                ui.end_row();
                ui.label("");
                crate::theme::checkbox(ui, &mut job.enabled, i18n::t("Attiva"));
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.label(RichText::new(i18n::tf(format!("Prossima esecuzione: {}", store::fmt_time(job.schedule.next_after(store::now()))))).weak());
            ui.separator();
            let valid = !job.name.trim().is_empty() && !job.conn.is_empty() && !matches!(&job.action, JobAction::Query { sql } if sql.trim().is_empty());
            ui.horizontal(|ui| {
                if ui.add_enabled(valid, crate::theme::primary(i18n::t("Salva"))).clicked() {
                    save = true;
                }
                if ui.button(i18n::t("Annulla")).clicked() {
                    close = true;
                }
            });
        });
        if save {
            let mut j = self.editing.take()?;
            j.name = j.name.trim().to_string();
            j.next_run = j.schedule.next_after(store::now());
            return Some(j);
        }
        if close {
            self.editing = None;
        }
        None
    }
}
