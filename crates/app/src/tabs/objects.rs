//! The "Oggetti" tab: the objects of the selected database/schema, Navicat-style.

use db_core::{ObjectKind, SchemaObject};
use egui::RichText;

use crate::app::{Action, Env, Location, ObjRef, icon};
use crate::icons::{self, Db, G};
use crate::dialogs::ExportSource;

pub fn object_menu(resp: &egui::Response, obj: &ObjRef, actions: &mut Vec<Action>) {
    resp.context_menu(|ui| {
        match obj.kind {
            ObjectKind::Table => {
                if icons::menu_item(ui, Db::Table, "Apri tabella").clicked() {
                    actions.push(Action::OpenTable(obj.clone()));
                }
                if icons::menu_item(ui, Db::Design, "Progetta tabella").clicked() {
                    actions.push(Action::DesignTable(obj.clone()));
                }
                ui.separator();
                if icons::menu_item(ui, Db::Import, "Importa…").clicked() {
                    actions.push(Action::Import(obj.clone()));
                }
                if icons::menu_item(ui, Db::Export, "Esporta…").clicked() {
                    actions.push(Action::Export(ExportSource::Table(obj.clone())));
                }
                ui.separator();
                if icons::menu_item(ui, G::Eraser, "Svuota tabella").clicked() {
                    actions.push(Action::Truncate(obj.clone()));
                }
                if icons::menu_item(ui, G::Delete, "Elimina tabella").clicked() {
                    actions.push(Action::Drop(obj.clone()));
                }
            }
            ObjectKind::View => {
                if icons::menu_item(ui, Db::View, "Apri vista").clicked() {
                    actions.push(Action::OpenTable(obj.clone()));
                }
                if icons::menu_item(ui, Db::Export, "Esporta…").clicked() {
                    actions.push(Action::Export(ExportSource::Table(obj.clone())));
                }
                if icons::menu_item(ui, G::Delete, "Elimina vista").clicked() {
                    actions.push(Action::Drop(obj.clone()));
                }
            }
            _ => {
                ui.label(RichText::new(obj.name.as_str()).weak());
            }
        }
        ui.separator();
        if icons::menu_item(ui, G::Copy, "Copia nome").clicked() {
            ui.ctx().copy_text(obj.name.clone());
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn ui(
    ui: &mut egui::Ui,
    loc: Option<&Location>,
    objects: Option<&Result<Vec<SchemaObject>, String>>,
    loading: bool,
    filter: &mut String,
    kinds: &[ObjectKind],
    selected: &mut Option<ObjRef>,
    env: &mut Env<'_>,
) {
    let Some(loc) = loc else {
        ui.centered_and_justified(|ui| ui.label(RichText::new("Apri una connessione e seleziona un database o uno schema").weak()));
        return;
    };
    let sel_table = selected.clone().filter(|o| o.kind == ObjectKind::Table && o.db == loc.db && o.schema == loc.schema && o.conn == loc.conn);
    ui.horizontal_wrapped(|ui| {
        if icons::button_ex(ui, Db::Table, "Apri tabella", false, sel_table.is_some()).clicked()
            && let Some(o) = sel_table.clone()
        {
            env.actions.push(Action::OpenTable(o));
        }
        if icons::button_ex(ui, Db::Design, "Progetta tabella", false, sel_table.is_some()).clicked()
            && let Some(o) = sel_table.clone()
        {
            env.actions.push(Action::DesignTable(o));
        }
        if icons::button(ui, Db::NewTable, "Nuova tabella").clicked() {
            env.actions.push(Action::NewTable(loc.clone()));
        }
        if icons::button_ex(ui, G::Delete, "Elimina tabella", false, sel_table.is_some()).clicked()
            && let Some(o) = sel_table.clone()
        {
            env.actions.push(Action::Drop(o));
        }
        ui.separator();
        if icons::button_ex(ui, Db::Import, "Importa", false, sel_table.is_some()).clicked()
            && let Some(o) = sel_table.clone()
        {
            env.actions.push(Action::Import(o));
        }
        if icons::button_ex(ui, Db::Export, "Esporta", false, sel_table.is_some()).clicked()
            && let Some(o) = sel_table.clone()
        {
            env.actions.push(Action::Export(ExportSource::Table(o)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::TextEdit::singleline(filter).hint_text("Cerca").desired_width(180.0));
            icons::show(ui, G::Search, 16.0);
            if icons::small(ui, Db::Refresh, "Aggiorna", true).clicked() {
                env.actions.push(Action::Refresh(loc.clone()));
            }
        });
    });
    ui.separator();
    if loading {
        ui.spinner();
        return;
    }
    let objs = match objects {
        Some(Ok(v)) => v,
        Some(Err(e)) => {
            ui.colored_label(egui::Color32::RED, e);
            return;
        }
        None => return,
    };
    let needle = filter.to_lowercase();
    let shown: Vec<&SchemaObject> = objs.iter().filter(|o| kinds.contains(&o.kind) && (needle.is_empty() || o.name.to_lowercase().contains(&needle))).collect();
    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(egui::Sense::click())
        .column(egui_extras::Column::initial(280.0).at_least(120.0))
        .column(egui_extras::Column::initial(110.0))
        .column(egui_extras::Column::initial(100.0))
        .column(egui_extras::Column::remainder())
        .header(22.0, |mut h| {
            for t in ["Nome", "Tipo", "Righe", "Commento"] {
                h.col(|ui| {
                    ui.strong(t);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, shown.len(), |mut row| {
                let Some(o) = shown.get(row.index()) else { return };
                let obj = ObjRef { conn: loc.conn.clone(), db: loc.db.clone(), schema: loc.schema.clone(), name: o.name.clone(), kind: o.kind };
                row.set_selected(selected.as_ref() == Some(&obj));
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        icons::show(ui, icon(o.kind), 16.0);
                        ui.label(o.name.as_str());
                    });
                });
                row.col(|ui| {
                    ui.label(o.kind.singular());
                });
                row.col(|ui| {
                    ui.label(o.rows.map(|r| r.to_string()).unwrap_or_default());
                });
                row.col(|ui| {
                    ui.label(o.comment.clone().unwrap_or_default());
                });
                let resp = row.response();
                if resp.clicked() {
                    *selected = Some(obj.clone());
                }
                if resp.double_clicked() && matches!(o.kind, ObjectKind::Table | ObjectKind::View) {
                    env.actions.push(Action::OpenTable(obj.clone()));
                }
                object_menu(&resp, &obj, env.actions);
            });
        });
}
