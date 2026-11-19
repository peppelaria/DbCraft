//! The "Oggetti" tab: the objects of the selected database/schema, Navicat-style.

use db_core::{ObjectKind, SchemaObject};
use egui::RichText;

use crate::app::{Action, Env, Location, ObjRef, icon};
use crate::icons::{self, Db, G};
use crate::dialogs::ExportSource;
use crate::i18n;

pub fn object_menu(resp: &egui::Response, obj: &ObjRef, fav: bool, actions: &mut Vec<Action>) {
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
        if icons::menu_item(ui, Db::Star, if fav { "Rimuovi dai preferiti" } else { "Aggiungi ai preferiti" }).clicked() {
            actions.push(Action::ToggleFavorite(obj.clone()));
        }
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
    favorites: &[crate::store::Favorite],
    env: &mut Env<'_>,
) {
    let Some(loc) = loc else {
        ui.centered_and_justified(|ui| ui.label(RichText::new(i18n::t("Apri una connessione e seleziona un database o uno schema")).weak()));
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
            ui.add(egui::TextEdit::singleline(filter).hint_text(i18n::t("Cerca")).desired_width(180.0));
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
    // the columns the server fills (as Navicat: Auto Increment, Modified Date, Data Length, Engine)
    let has = |f: &dyn Fn(&SchemaObject) -> bool| shown.iter().any(|o| f(o));
    let mut cols: Vec<(&str, Col)> = vec![("Nome", Col::Name)];
    if has(&|o| o.auto_increment.is_some()) {
        cols.push(("Auto Increment", Col::AutoInc));
    }
    if has(&|o| o.modified.is_some()) {
        cols.push(("Data modifica", Col::Modified));
    }
    if has(&|o| o.data_length.is_some()) {
        cols.push(("Dimensione dati", Col::DataLength));
    }
    if has(&|o| o.engine.is_some()) {
        cols.push(("Engine", Col::Engine));
    }
    if kinds.len() > 1 {
        cols.push(("Tipo", Col::Kind));
    }
    cols.push(("Righe", Col::Rows));
    cols.push(("Commento", Col::Comment));
    let n = cols.len();
    // wider than the window: it scrolls sideways, as Navicat's list
    egui::ScrollArea::horizontal().id_salt("objects_h").show(ui, |ui| {
    let mut table = egui_extras::TableBuilder::new(ui).striped(true).resizable(true).sense(egui::Sense::click_and_drag()).auto_shrink([false, false]);
    for (i, (_, c)) in cols.iter().enumerate() {
        table = table.column(match c {
            Col::Name => egui_extras::Column::initial(220.0).at_least(120.0).clip(true),
            _ if i + 1 == n => egui_extras::Column::remainder().at_least(140.0),
            Col::Modified => egui_extras::Column::initial(150.0).clip(true),
            _ => egui_extras::Column::initial(105.0).clip(true),
        });
    }
    let numeric = |c: &Col| matches!(c, Col::AutoInc | Col::DataLength | Col::Rows);
    table
        .header(22.0, |mut h| {
            for (t, c) in &cols {
                h.col(|ui| {
                    if numeric(c) {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| ui.strong(i18n::t(t)));
                    } else {
                        ui.strong(i18n::t(t));
                    }
                });
            }
        })
        .body(|body| {
            body.rows(22.0, shown.len(), |mut row| {
                let Some(o) = shown.get(row.index()) else { return };
                let obj = ObjRef { conn: loc.conn.clone(), db: loc.db.clone(), schema: loc.schema.clone(), name: o.name.clone(), kind: o.kind };
                let fav = crate::app::is_favorite(favorites, &obj);
                row.set_selected(selected.as_ref() == Some(&obj));
                for (_, c) in &cols {
                    row.col(|ui| {
                        let text = match c {
                            Col::Name => {
                                ui.horizontal(|ui| {
                                    icons::show(ui, icon(o.kind), 16.0);
                                    ui.label(o.name.as_str());
                                    if fav {
                                        icons::show(ui, Db::Star, 11.0);
                                    }
                                });
                                return;
                            }
                            Col::Kind => i18n::t(o.kind.singular()).to_string(),
                            Col::AutoInc => o.auto_increment.map(|v| v.to_string()).unwrap_or_default(),
                            Col::Modified => o.modified.clone().unwrap_or_default(),
                            Col::DataLength => o.data_length.map(|b| format!("{} KB", (b + 1023) / 1024)).unwrap_or_default(),
                            Col::Engine => o.engine.clone().unwrap_or_default(),
                            Col::Rows => o.rows.map(|r| r.to_string()).unwrap_or_default(),
                            Col::Comment => o.comment.clone().unwrap_or_default(),
                        };
                        if numeric(c) {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| ui.label(text));
                        } else {
                            ui.add(egui::Label::new(text).truncate());
                        }
                    });
                }
                let resp = row.response();
                if resp.clicked() {
                    *selected = Some(obj.clone());
                }
                if resp.double_clicked() && matches!(o.kind, ObjectKind::Table | ObjectKind::View) {
                    env.actions.push(Action::OpenTable(obj.clone()));
                }
                // onto the AI chat, to talk about it
                if matches!(o.kind, ObjectKind::Table | ObjectKind::View) {
                    resp.dnd_set_drag_payload(obj.clone());
                }
                object_menu(&resp, &obj, fav, env.actions);
            });
        });
    });
}

enum Col {
    Name,
    Kind,
    AutoInc,
    Modified,
    DataLength,
    Engine,
    Rows,
    Comment,
}

/// The "Query" view of the Objects tab (as in Navicat): the queries saved for this
/// database/schema, with Apri / Nuova / Elimina.
pub fn queries_ui(ui: &mut egui::Ui, loc: Option<&Location>, filter: &mut String, selected: &mut Option<std::path::PathBuf>, env: &mut Env<'_>) {
    let Some(loc) = loc else {
        ui.centered_and_justified(|ui| ui.label(RichText::new(i18n::t("Apri una connessione e seleziona un database o uno schema")).weak()));
        return;
    };
    let dir = crate::store::place_dir("queries", &loc.conn, &loc.db, loc.schema.as_deref());
    let files = crate::store::list_files(&dir, "sql");
    let sel = selected.clone().filter(|p| files.iter().any(|f| &f.path == p));
    ui.horizontal_wrapped(|ui| {
        if icons::button_ex(ui, Db::Query, "Apri query", false, sel.is_some()).clicked()
            && let Some(p) = sel.clone()
        {
            env.actions.push(Action::OpenQueryFile(loc.clone(), p));
        }
        if icons::button(ui, G::NewFile, "Nuova query").clicked() {
            env.actions.push(Action::NewQuery(loc.clone()));
        }
        if icons::button_ex(ui, G::Delete, "Elimina query", false, sel.is_some()).clicked()
            && let Some(p) = sel.clone()
        {
            env.actions.push(Action::DeleteQueryFile(p));
        }
        ui.separator();
        if icons::button(ui, G::Folder, "Mostra cartella").clicked() {
            let _ = std::fs::create_dir_all(&dir);
            crate::selfupdate::open_url(&dir.to_string_lossy());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::TextEdit::singleline(filter).hint_text(i18n::t("Cerca")).desired_width(180.0));
            icons::show(ui, G::Search, 16.0);
        });
    });
    ui.separator();
    let needle = filter.to_lowercase();
    let shown: Vec<&crate::store::FileEntry> = files.iter().filter(|f| needle.is_empty() || f.name.to_lowercase().contains(&needle)).collect();
    if shown.is_empty() {
        ui.label(RichText::new(i18n::t("Nessuna query salvata. Le query salvate dalla scheda Query (⌘S) finiscono qui.")).weak());
    }
    egui_extras::TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(egui::Sense::click())
        .auto_shrink([false, false])
        .column(egui_extras::Column::initial(280.0).at_least(120.0))
        .column(egui_extras::Column::initial(160.0))
        .column(egui_extras::Column::remainder())
        .header(22.0, |mut h| {
            for t in ["Nome", "Data modifica", "Dimensione"] {
                h.col(|ui| {
                    ui.strong(i18n::t(t));
                });
            }
        })
        .body(|body| {
            body.rows(22.0, shown.len(), |mut row| {
                let Some(f) = shown.get(row.index()) else { return };
                row.set_selected(sel.as_ref() == Some(&f.path));
                row.col(|ui| {
                    ui.horizontal(|ui| {
                        icons::show(ui, Db::Query, 16.0);
                        ui.label(f.name.as_str());
                    });
                });
                row.col(|ui| {
                    ui.label(f.modified.as_str());
                });
                row.col(|ui| {
                    ui.label(crate::tabs::backup::fmt_size(f.size));
                });
                let r = row.response();
                if r.clicked() {
                    *selected = Some(f.path.clone());
                }
                if r.double_clicked() {
                    env.actions.push(Action::OpenQueryFile(loc.clone(), f.path.clone()));
                }
                r.context_menu(|ui| {
                    if icons::menu_item(ui, Db::Query, "Apri query").clicked() {
                        env.actions.push(Action::OpenQueryFile(loc.clone(), f.path.clone()));
                    }
                    if icons::menu_item(ui, G::Delete, "Elimina query").clicked() {
                        env.actions.push(Action::DeleteQueryFile(f.path.clone()));
                    }
                });
            });
        });
}
