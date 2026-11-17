//! Table viewer/editor: a paged grid with filter, server-side sorting and pending edits that
//! are applied in one transaction.

use db_core::{Dialect, ObjectKind, PageRequest, ResultSet, RowChange, TableInfo, Value};
use dbgrid::{DataGrid, GridColumn, GridEvent, SortDir};
use egui::RichText;

use crate::app::{Action, Env, ObjRef};
use crate::backend::{Msg, TabId};
use crate::dialogs::ExportSource;
use crate::models::{Edits, TableModel, grid_columns};

pub struct TableTab {
    pub id: TabId,
    pub obj: ObjRef,
    info: Option<TableInfo>,
    data: ResultSet,
    columns: Vec<GridColumn>,
    pub edits: Edits,
    grid: DataGrid,
    page: u64,
    page_size: u64,
    total: Option<u64>,
    filter: String,
    applied_filter: Option<String>,
    sort: Option<(String, bool)>,
    loading: bool,
    saving: bool,
    error: Option<String>,
    last_sql: String,
}

impl TableTab {
    pub fn new(id: TabId, obj: ObjRef) -> Self {
        TableTab {
            id,
            grid: DataGrid::new(("table", id)),
            obj,
            info: None,
            data: ResultSet::default(),
            columns: Vec::new(),
            edits: Edits::default(),
            page: 0,
            page_size: 1000,
            total: None,
            filter: String::new(),
            applied_filter: None,
            sort: None,
            loading: false,
            saving: false,
            error: None,
            last_sql: String::new(),
        }
    }

    fn request(&self) -> PageRequest {
        PageRequest {
            schema: self.obj.schema.clone(),
            table: self.obj.name.clone(),
            filter: self.applied_filter.clone(),
            order: self.sort.clone(),
            offset: self.page * self.page_size,
            limit: self.page_size,
        }
    }

    pub fn reload(&mut self, env: &mut Env<'_>, with_info: bool) {
        let Some(cfg) = env.cfg(&self.obj.conn).cloned() else { return };
        self.loading = true;
        self.error = None;
        let req = self.request();
        self.last_sql = Dialect::from(cfg.kind).select_page(&req);
        let (tab, obj) = (self.id, self.obj.clone());
        let with_info = with_info || self.info.is_none();
        env.backend.with_driver(&cfg, &self.obj.db, move |d| async move {
            let d = match d {
                Ok(d) => d,
                Err(e) => return Msg::TableData { tab, info: None, page: Err(e), total: None },
            };
            let info = if with_info { Some(d.describe_table(obj.schema.as_deref(), &obj.name).await) } else { None };
            let page = d.fetch_page(&req).await;
            let total = d.count_rows(obj.schema.as_deref(), &obj.name, req.filter.as_deref()).await.ok();
            Msg::TableData { tab, info, page, total }
        });
    }

    pub fn handle(&mut self, msg: Msg, env: &mut Env<'_>) {
        match msg {
            Msg::TableData { info, page, total, .. } => {
                self.loading = false;
                if let Some(info) = info {
                    match info {
                        Ok(i) => self.info = Some(i),
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                match page {
                    Ok(rs) => {
                        let reset = self.columns.iter().map(|c| &c.name).ne(rs.columns.iter().map(|c| &c.name));
                        self.columns = grid_columns(&rs, self.info.as_ref());
                        self.data = rs;
                        self.edits = Edits::default();
                        if reset && !self.data.columns.is_empty() {
                            self.grid.reset();
                        }
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
                self.total = total;
            }
            Msg::Applied { res, .. } => {
                self.saving = false;
                match res {
                    Ok(n) => {
                        env.actions.push(Action::Notice(format!("{n} record modificati"), false));
                        self.reload(env, false);
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        env.actions.push(Action::Notice(format!("Modifiche non applicate: {e}"), true));
                    }
                }
            }
            _ => {}
        }
    }

    fn editable(&self) -> bool {
        self.obj.kind == ObjectKind::Table && self.info.is_some() && !self.loading && !self.saving
    }

    fn key_of(&self, row: usize) -> Vec<(String, Value)> {
        let pk: Vec<String> = self.info.as_ref().map(TableInfo::primary_key).unwrap_or_default();
        let original = self.data.rows.get(row);
        let cols: Vec<(usize, &str)> = self.data.columns.iter().enumerate().map(|(i, c)| (i, c.name.as_str())).collect();
        let pick = |(i, name): &(usize, &str)| (name.to_string(), original.and_then(|r| r.get(*i)).cloned().unwrap_or_default());
        if pk.is_empty() {
            cols.iter().filter(|(i, _)| !matches!(original.and_then(|r| r.get(*i)), Some(Value::Bytes(_)))).map(pick).collect()
        } else {
            cols.iter().filter(|(_, n)| pk.iter().any(|p| p == n)).map(pick).collect()
        }
    }

    fn changes(&self) -> Vec<RowChange> {
        let mut out = Vec::new();
        let mut deleted: Vec<usize> = self.edits.deleted.iter().copied().collect();
        deleted.sort_unstable();
        for r in deleted {
            out.push(RowChange::Delete { key: self.key_of(r) });
        }
        let mut rows: Vec<usize> = self.edits.cells.keys().map(|k| k.0).filter(|r| !self.edits.deleted.contains(r)).collect();
        rows.sort_unstable();
        rows.dedup();
        for r in rows {
            let mut set: Vec<(usize, Value)> = self.edits.cells.iter().filter(|(k, _)| k.0 == r).map(|(k, v)| (k.1, v.clone())).collect();
            set.sort_by_key(|x| x.0);
            let set = set.into_iter().filter_map(|(c, v)| self.data.columns.get(c).map(|m| (m.name.clone(), v))).collect();
            out.push(RowChange::Update { key: self.key_of(r), set });
        }
        for row in &self.edits.inserted {
            let values = row.iter().enumerate().filter_map(|(c, v)| v.clone().and_then(|v| self.columns.get(c).map(|m| (m.name.clone(), v)))).collect();
            out.push(RowChange::Insert { values });
        }
        out
    }

    fn apply(&mut self, env: &mut Env<'_>) {
        let Some(cfg) = env.cfg(&self.obj.conn).cloned() else { return };
        let changes = self.changes();
        if changes.is_empty() {
            return;
        }
        self.saving = true;
        let (tab, obj) = (self.id, self.obj.clone());
        env.backend.with_driver(&cfg, &self.obj.db, move |d| async move {
            let res = match d {
                Ok(d) => d.apply_changes(obj.schema.as_deref(), &obj.name, &changes).await,
                Err(e) => Err(e),
            };
            Msg::Applied { tab, res }
        });
    }

    fn on_grid_event(&mut self, ev: GridEvent, env: &mut Env<'_>) {
        let n = self.data.rows.len();
        match ev {
            GridEvent::CellEdited { row, col, value } => {
                let v = value.map_or(Value::Null, Value::Text);
                if row >= n {
                    if let Some(cell) = self.edits.inserted.get_mut(row - n).and_then(|r| r.get_mut(col)) {
                        *cell = Some(v);
                    }
                } else {
                    let original = self.data.rows.get(row).and_then(|r| r.get(col));
                    let same = original.is_some_and(|o| *o == v || (!o.is_null() && !v.is_null() && o.to_string() == v.to_string()));
                    if same {
                        self.edits.cells.remove(&(row, col));
                    } else {
                        self.edits.cells.insert((row, col), v);
                    }
                }
            }
            GridEvent::InsertRow => {
                self.edits.inserted.push(vec![None; self.columns.len()]);
                self.grid.select_cell((n + self.edits.inserted.len() - 1) as u32, 0);
            }
            GridEvent::DeleteRows(rows) => {
                let mut ins: Vec<usize> = rows.iter().filter(|r| **r >= n).map(|r| r - n).collect();
                ins.sort_unstable_by(|a, b| b.cmp(a));
                for i in ins {
                    if i < self.edits.inserted.len() {
                        self.edits.inserted.remove(i);
                    }
                }
                for r in rows.into_iter().filter(|r| *r < n) {
                    if !self.edits.deleted.remove(&r) {
                        self.edits.deleted.insert(r);
                    }
                }
            }
            GridEvent::SortRequested { col, dir } => {
                if !self.edits.is_empty() {
                    env.actions.push(Action::Notice("Applica o annulla le modifiche prima di ordinare".into(), true));
                    return;
                }
                self.sort = dir.and_then(|d| self.columns.get(col).map(|c| (c.name.clone(), d == SortDir::Desc)));
                self.grid.sort = dir.map(|d| (col, d));
                self.page = 0;
                self.reload(env, false);
            }
            GridEvent::FilterByValue { col, value } => {
                let Some(c) = self.columns.get(col) else { return };
                let d = env.dialect(&self.obj.conn);
                self.filter = match value {
                    None => format!("{} IS NULL", d.quote(&c.name)),
                    Some(v) => format!("{} = {}", d.quote(&c.name), d.string_literal(&v)),
                };
                self.applied_filter = Some(self.filter.clone());
                self.page = 0;
                self.reload(env, false);
            }
            GridEvent::Activated { .. } | GridEvent::SelectionChanged => {}
        }
    }

    pub fn status_text(&self) -> String {
        let n = self.data.rows.len() as u64;
        let first = if n == 0 { 0 } else { self.page * self.page_size + 1 };
        let mut s = format!("{}   —   Record {}–{}", self.last_sql, first, self.page * self.page_size + n);
        if let Some(t) = self.total {
            s.push_str(&format!(" di {t}"));
        }
        if !self.edits.is_empty() {
            s.push_str(&format!("   •   {} modifiche in sospeso", self.edits.count()));
        }
        s
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        let editable = self.editable();
        let dirty = !self.edits.is_empty();
        let mut reload = false;
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(editable, egui::Button::new("➕").small()).on_hover_text("Aggiungi record").clicked() {
                self.on_grid_event(GridEvent::InsertRow, env);
            }
            if ui.add_enabled(editable, egui::Button::new("➖").small()).on_hover_text("Elimina record selezionati").clicked() {
                let rows = self.grid.sel.rows((self.data.rows.len() + self.edits.inserted.len()) as u32, self.columns.len() as u32);
                self.on_grid_event(GridEvent::DeleteRows(rows), env);
            }
            if ui.add_enabled(dirty && editable, egui::Button::new("✔").small()).on_hover_text("Applica modifiche").clicked() {
                self.apply(env);
            }
            if ui.add_enabled(dirty, egui::Button::new("✖").small()).on_hover_text("Annulla modifiche").clicked() {
                self.edits = Edits::default();
            }
            if ui.add_enabled(!self.loading, egui::Button::new("🔄").small()).on_hover_text("Aggiorna").clicked() {
                reload = true;
            }
            ui.separator();
            ui.label("WHERE");
            let resp = ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("es. id > 10 AND nome LIKE 'A%'").desired_width(ui.available_width().clamp(120.0, 320.0)).font(egui::TextStyle::Monospace));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.small_button("Applica filtro").clicked() || enter {
                self.applied_filter = Some(self.filter.trim().to_string()).filter(|f| !f.is_empty());
                self.page = 0;
                reload = true;
            }
            if self.applied_filter.is_some() && ui.small_button("× Rimuovi").clicked() {
                self.filter.clear();
                self.applied_filter = None;
                self.page = 0;
                reload = true;
            }
            ui.separator();
            if ui.small_button("📤 Esporta").clicked() {
                env.actions.push(Action::Export(ExportSource::Table(self.obj.clone())));
            }
            if self.obj.kind == ObjectKind::Table && ui.small_button("🖊 Progetta").clicked() {
                env.actions.push(Action::DesignTable(self.obj.clone()));
            }
            if self.loading || self.saving {
                ui.spinner();
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::RED, e);
        }
        if self.info.as_ref().is_some_and(|i| i.primary_key().is_empty()) && self.obj.kind == ObjectKind::Table {
            ui.label(RichText::new("⚠ La tabella non ha chiave primaria: le modifiche usano tutti i valori della riga per identificarla.").small().weak());
        }

        // Pager at the bottom.
        let pages = self.total.map(|t| t.div_ceil(self.page_size).max(1));
        egui::Panel::bottom(egui::Id::unique(("pager", self.id))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let last = pages.map_or(u64::MAX, |p| p - 1);
                    if ui.add_enabled(!dirty && self.page < last && pages.is_some(), egui::Button::new("⏭").small()).clicked() {
                        self.page = last;
                        reload = true;
                    }
                    if ui.add_enabled(!dirty && self.page < last && self.data.rows.len() as u64 == self.page_size, egui::Button::new("▶").small()).clicked() {
                        self.page += 1;
                        reload = true;
                    }
                    ui.label(format!("Pagina {}{}", self.page + 1, pages.map(|p| format!(" / {p}")).unwrap_or_default()));
                    if ui.add_enabled(!dirty && self.page > 0, egui::Button::new("◀").small()).clicked() {
                        self.page -= 1;
                        reload = true;
                    }
                    if ui.add_enabled(!dirty && self.page > 0, egui::Button::new("⏮").small()).clicked() {
                        self.page = 0;
                        reload = true;
                    }
                    ui.separator();
                    let before = self.page_size;
                    egui::ComboBox::from_id_salt(("ps", self.id)).selected_text(format!("{} righe", self.page_size)).show_ui(ui, |ui| {
                        for n in [100, 500, 1000, 5000, 10000, 50000] {
                            ui.selectable_value(&mut self.page_size, n, n.to_string());
                        }
                    });
                    if before != self.page_size && !dirty {
                        self.page = 0;
                        reload = true;
                    }
                });
            });
        });

        let events = {
            let model = TableModel { rs: &self.data, columns: &self.columns, edits: &self.edits, editable };
            self.grid.show(ui, &model)
        };
        for ev in events {
            self.on_grid_event(ev, env);
        }
        if reload && !self.loading {
            self.reload(env, false);
        }
    }
}
