//! Table designer: fields, indexes and foreign keys, with a live SQL preview of the
//! CREATE/ALTER statements for the connection's dialect.

use db_core::{ColumnDef, Dialect, ForeignKeyDef, IndexDef, ObjectKind, TableDesign};
use egui::RichText;

use crate::app::{Action, Env, Location, ObjRef};
use crate::backend::{Msg, TabId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sub {
    Fields,
    Indexes,
    ForeignKeys,
    Sql,
}

pub struct DesignerTab {
    pub id: TabId,
    loc: Location,
    obj: Option<ObjRef>,
    dialect: Dialect,
    original: Option<TableDesign>,
    design: TableDesign,
    sub: Sub,
    selected: usize,
    loading: bool,
    saving: bool,
    error: Option<String>,
}

const FK_ACTIONS: [&str; 5] = ["", "CASCADE", "SET NULL", "RESTRICT", "NO ACTION"];

impl DesignerTab {
    pub fn new_table(id: TabId, loc: Location, dialect: Dialect) -> Self {
        let id_type = match dialect {
            Dialect::Sqlite => "INTEGER",
            Dialect::Postgres => "integer",
            _ => "int",
        };
        let design = TableDesign {
            schema: loc.schema.clone(),
            name: String::new(),
            columns: vec![ColumnDef { name: "id".into(), data_type: id_type.into(), not_null: true, primary_key: true, auto_increment: true, ..Default::default() }],
            ..Default::default()
        };
        DesignerTab { id, loc, obj: None, dialect, original: None, design, sub: Sub::Fields, selected: 0, loading: false, saving: false, error: None }
    }

    pub fn edit(id: TabId, obj: ObjRef, dialect: Dialect) -> Self {
        let loc = Location { conn: obj.conn.clone(), db: obj.db.clone(), schema: obj.schema.clone() };
        let design = TableDesign { schema: obj.schema.clone(), name: obj.name.clone(), ..Default::default() };
        DesignerTab { id, loc, obj: Some(obj), dialect, original: None, design, sub: Sub::Fields, selected: 0, loading: true, saving: false, error: None }
    }

    pub fn load(&mut self, env: &mut Env<'_>) {
        let (Some(obj), Some(cfg)) = (self.obj.clone(), env.cfg(&self.loc.conn).cloned()) else { return };
        self.loading = true;
        let tab = self.id;
        env.backend.with_driver(&cfg, &obj.db, move |d| async move {
            let res = match d {
                Ok(d) => d.describe_table(obj.schema.as_deref(), &obj.name).await,
                Err(e) => Err(e),
            };
            Msg::DesignLoaded { tab, res }
        });
    }

    pub fn handle(&mut self, msg: Msg, env: &mut Env<'_>) {
        match msg {
            Msg::DesignLoaded { res, .. } => {
                self.loading = false;
                match res {
                    Ok(info) => {
                        let d = TableDesign::from_info(&info);
                        self.original = Some(d.clone());
                        self.design = d;
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            Msg::DesignSaved { res, .. } => {
                self.saving = false;
                match res {
                    Ok(()) => {
                        let mut d = self.design.clone();
                        for c in &mut d.columns {
                            c.original = Some(c.name.clone());
                        }
                        self.design = d.clone();
                        self.original = Some(d);
                        self.error = None;
                        let obj = ObjRef { conn: self.loc.conn.clone(), db: self.loc.db.clone(), schema: self.loc.schema.clone(), name: self.design.name.clone(), kind: ObjectKind::Table };
                        self.obj = Some(obj);
                        env.actions.push(Action::SetTitle(self.id, format!("{} (Progetta)", self.design.name)));
                        env.actions.push(Action::Refresh(self.loc.clone()));
                        env.actions.push(Action::Notice(format!("Tabella \"{}\" salvata", self.design.name), false));
                        // Reload to pick up what the server actually stored.
                        self.load(env);
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                        env.actions.push(Action::Notice(format!("Salvataggio fallito: {e}"), true));
                    }
                }
            }
            _ => {}
        }
    }

    fn statements(&self) -> Vec<String> {
        match &self.original {
            Some(o) => self.dialect.alter_table(o, &self.design),
            None => self.dialect.create_table(&self.design),
        }
    }

    fn save(&mut self, env: &mut Env<'_>) {
        if self.design.name.trim().is_empty() {
            self.error = Some("Inserisci il nome della tabella".into());
            return;
        }
        if self.design.columns.iter().any(|c| c.name.trim().is_empty()) {
            self.error = Some("Ogni campo deve avere un nome".into());
            return;
        }
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        let stmts = self.statements();
        if stmts.is_empty() {
            env.actions.push(Action::Notice("Nessuna modifica da salvare".into(), false));
            return;
        }
        self.saving = true;
        let tab = self.id;
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let d = match d {
                Ok(d) => d,
                Err(e) => return Msg::DesignSaved { tab, res: Err(e.to_string()) },
            };
            for s in stmts {
                if s.starts_with("--") {
                    continue;
                }
                if let Err(e) = d.execute(&s).await {
                    return Msg::DesignSaved { tab, res: Err(format!("{e}\n\n{s}")) };
                }
            }
            Msg::DesignSaved { tab, res: Ok(()) }
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>, _id: TabId) {
        if self.loading {
            ui.spinner();
            return;
        }
        let dirty = self.original.as_ref() != Some(&self.design);
        ui.horizontal(|ui| {
            if ui.add_enabled(dirty && !self.saving, egui::Button::new(RichText::new("💾 Salva").strong())).clicked() {
                self.save(env);
            }
            ui.separator();
            match self.sub {
                Sub::Fields => {
                    if ui.button("➕ Aggiungi campo").clicked() {
                        self.design.columns.push(ColumnDef { data_type: self.dialect.types().get(1).copied().unwrap_or("TEXT").into(), ..Default::default() });
                        self.selected = self.design.columns.len() - 1;
                    }
                    if ui.button("↥ Inserisci campo").clicked() {
                        let at = self.selected.min(self.design.columns.len());
                        self.design.columns.insert(at, ColumnDef { data_type: self.dialect.types().get(1).copied().unwrap_or("TEXT").into(), ..Default::default() });
                    }
                    if ui.add_enabled(!self.design.columns.is_empty(), egui::Button::new("➖ Elimina campo")).clicked() && self.selected < self.design.columns.len() {
                        self.design.columns.remove(self.selected);
                        self.selected = self.selected.min(self.design.columns.len().saturating_sub(1));
                    }
                    if ui.add_enabled(self.selected > 0, egui::Button::new("⬆")).clicked() {
                        self.design.columns.swap(self.selected, self.selected - 1);
                        self.selected -= 1;
                    }
                    if ui.add_enabled(self.selected + 1 < self.design.columns.len(), egui::Button::new("⬇")).clicked() {
                        self.design.columns.swap(self.selected, self.selected + 1);
                        self.selected += 1;
                    }
                }
                Sub::Indexes => {
                    if ui.button("➕ Aggiungi indice").clicked() {
                        let n = self.design.indexes.len() + 1;
                        self.design.indexes.push(IndexDef { name: format!("idx_{}_{n}", self.design.name), ..Default::default() });
                    }
                }
                Sub::ForeignKeys => {
                    if ui.button("➕ Aggiungi chiave esterna").clicked() {
                        let n = self.design.foreign_keys.len() + 1;
                        self.design.foreign_keys.push(ForeignKeyDef { name: format!("fk_{}_{n}", self.design.name), ..Default::default() });
                    }
                }
                Sub::Sql => {}
            }
            if self.saving {
                ui.spinner();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Nome tabella:");
            ui.add(egui::TextEdit::singleline(&mut self.design.name).desired_width(240.0));
            if let Some(s) = &self.loc.schema {
                ui.label(RichText::new(format!("schema: {s}")).weak());
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C), e);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.sub, Sub::Fields, "Campi");
            ui.selectable_value(&mut self.sub, Sub::Indexes, "Indici");
            ui.selectable_value(&mut self.sub, Sub::ForeignKeys, "Chiavi esterne");
            ui.selectable_value(&mut self.sub, Sub::Sql, "Anteprima SQL");
        });
        ui.separator();
        match self.sub {
            Sub::Fields => self.fields_ui(ui),
            Sub::Indexes => self.indexes_ui(ui),
            Sub::ForeignKeys => self.fks_ui(ui),
            Sub::Sql => {
                let mut text = self.statements().join(";\n\n");
                if !text.is_empty() {
                    text.push(';');
                } else {
                    text = "-- nessuna modifica".into();
                }
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut text).code_editor().desired_width(f32::INFINITY).interactive(true));
                });
            }
        }
    }

    fn fields_ui(&mut self, ui: &mut egui::Ui) {
        let types = self.dialect.types();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("fields", self.id)).striped(true).num_columns(8).spacing([8.0, 4.0]).show(ui, |ui| {
                for h in ["", "Nome", "Tipo", "Non null", "Chiave", "Auto inc.", "Predefinito", "Commento"] {
                    ui.strong(h);
                }
                ui.end_row();
                for (i, c) in self.design.columns.iter_mut().enumerate() {
                    if ui.selectable_label(self.selected == i, if self.selected == i { "▶" } else { " " }).clicked() {
                        self.selected = i;
                    }
                    let r = ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(160.0));
                    if r.gained_focus() {
                        self.selected = i;
                    }
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut c.data_type).desired_width(130.0));
                        egui::ComboBox::from_id_salt(("ty", self.id, i)).width(18.0).selected_text("").show_ui(ui, |ui| {
                            for t in types {
                                if ui.selectable_label(c.data_type == *t, *t).clicked() {
                                    c.data_type = (*t).to_string();
                                }
                            }
                        });
                    });
                    ui.checkbox(&mut c.not_null, "");
                    ui.checkbox(&mut c.primary_key, "🔑");
                    ui.checkbox(&mut c.auto_increment, "");
                    ui.add(egui::TextEdit::singleline(&mut c.default).desired_width(120.0).hint_text("es. 0, 'abc', CURRENT_TIMESTAMP"));
                    ui.add_enabled(self.dialect != Dialect::Sqlite && self.dialect != Dialect::SqlServer, egui::TextEdit::singleline(&mut c.comment).desired_width(180.0));
                    ui.end_row();
                }
            });
        });
    }

    fn indexes_ui(&mut self, ui: &mut egui::Ui) {
        let mut remove = None;
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("idx", self.id)).striped(true).num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
                for h in ["Nome", "Campi (separati da virgola)", "Unico", ""] {
                    ui.strong(h);
                }
                ui.end_row();
                for (i, x) in self.design.indexes.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut x.name).desired_width(200.0));
                    ui.add(egui::TextEdit::singleline(&mut x.columns).desired_width(260.0));
                    ui.checkbox(&mut x.unique, "");
                    if ui.small_button("🗑").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(i) = remove {
            self.design.indexes.remove(i);
        }
    }

    fn fks_ui(&mut self, ui: &mut egui::Ui) {
        let mut remove = None;
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("fk", self.id)).striped(true).num_columns(7).spacing([8.0, 4.0]).show(ui, |ui| {
                for h in ["Nome", "Campi", "Tabella di riferimento", "Campi di riferimento", "Su eliminazione", "Su aggiornamento", ""] {
                    ui.strong(h);
                }
                ui.end_row();
                for (i, f) in self.design.foreign_keys.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(160.0));
                    ui.add(egui::TextEdit::singleline(&mut f.columns).desired_width(140.0));
                    ui.add(egui::TextEdit::singleline(&mut f.ref_table).desired_width(140.0));
                    ui.add(egui::TextEdit::singleline(&mut f.ref_columns).desired_width(140.0));
                    for (k, slot) in [&mut f.on_delete, &mut f.on_update].into_iter().enumerate() {
                        egui::ComboBox::from_id_salt(("fka", self.id, i, k)).selected_text(slot.as_str()).show_ui(ui, |ui| {
                            for a in FK_ACTIONS {
                                ui.selectable_value(slot, a.to_string(), if a.is_empty() { "(predefinito)" } else { a });
                            }
                        });
                    }
                    if ui.small_button("🗑").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(i) = remove {
            self.design.foreign_keys.remove(i);
        }
    }
}
