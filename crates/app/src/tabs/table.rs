//! Table viewer/editor: a paged grid with a visual filter & sort, column visibility, a cell
//! editor side panel, grid / form / text views, and edits that are either saved as you leave a
//! record (auto-commit) or held until Commit (transaction mode).

use crate::icons::{self, Db, G};
use std::collections::BTreeSet;

use db_core::{Dialect, ObjectKind, PageRequest, ResultSet, RowChange, TableInfo, Value};
use dbgrid::datetime::{self, DateKind, PickerOutcome, PickerState};
use dbgrid::{ColumnKind, DataGrid, GridColumn, GridEvent, SortDir};
use egui::RichText;

use crate::app::{Action, Env, ObjRef};
use crate::backend::{Msg, TabId};
use crate::dialogs::ExportSource;
use crate::models::{Edits, TableModel, grid_columns};
use crate::tabs::filter::{FilterSort, Item, Join, Node, Op, Sort};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Grid,
    Form,
    Text,
}

pub struct TableTab {
    pub id: TabId,
    pub obj: ObjRef,
    info: Option<TableInfo>,
    data: ResultSet,
    /// All columns of the result, in result order.
    columns: Vec<GridColumn>,
    /// Visible columns and their result-set indexes.
    vis_cols: Vec<GridColumn>,
    col_map: Vec<usize>,
    hidden: BTreeSet<String>,
    pub edits: Edits,
    grid: DataGrid,
    page: u64,
    page_size: u64,
    total: Option<u64>,
    fs: FilterSort,
    show_filter: bool,
    applied_where: Option<String>,
    order: Vec<(String, bool)>,
    loading: bool,
    saving: bool,
    error: Option<String>,
    last_sql: String,
    view: View,
    /// Auto-commit (default): a record is saved when you leave it. Off = transaction mode.
    autocommit: bool,
    confirm_delete: Option<usize>,
    show_cell_editor: bool,
    cell_key: Option<(usize, usize)>,
    cell_buf: String,
    form_row: Option<usize>,
    form_bufs: Vec<String>,
    text_cache: Option<String>,
    debug_flags: Vec<String>,
    /// Form field (index in `col_map`) whose date picker is open, and its anchor.
    form_picker: Option<(usize, egui::Rect)>,
    /// Inline picker of the cell editor (date/time columns).
    cell_picker: Option<PickerState>,
}

fn value_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        other => other.to_string(),
    }
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
            vis_cols: Vec::new(),
            col_map: Vec::new(),
            hidden: BTreeSet::new(),
            edits: Edits::default(),
            page: 0,
            page_size: 1000,
            total: None,
            fs: FilterSort::default(),
            show_filter: false,
            applied_where: None,
            order: Vec::new(),
            loading: false,
            saving: false,
            error: None,
            last_sql: String::new(),
            view: View::Grid,
            autocommit: true,
            confirm_delete: None,
            show_cell_editor: false,
            cell_key: None,
            cell_buf: String::new(),
            form_row: None,
            form_bufs: Vec::new(),
            text_cache: None,
            debug_flags: Vec::new(),
            form_picker: None,
            cell_picker: None,
        }
    }

    /// Scripted UI state for screenshots (`--table-ui`). The filter flag adds a sample
    /// condition on the second column.
    pub fn debug_setup(&mut self, flags: &[String]) {
        for f in flags {
            match f.as_str() {
                "filter" => self.show_filter = true,
                "cell" => self.show_cell_editor = true,
                "form" => self.view = View::Form,
                "text" => self.view = View::Text,
                "tx" => self.autocommit = false,
                _ => {}
            }
        }
        self.debug_flags = flags.to_vec();
    }

    fn request(&self) -> PageRequest {
        PageRequest {
            schema: self.obj.schema.clone(),
            table: self.obj.name.clone(),
            filter: self.applied_where.clone(),
            order: self.order.clone(),
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

    fn rebuild_visible(&mut self) {
        self.col_map = self.columns.iter().enumerate().filter(|(_, c)| !self.hidden.contains(&c.name)).map(|(i, _)| i).collect();
        self.vis_cols = self.col_map.iter().filter_map(|&i| self.columns.get(i).cloned()).collect();
        self.grid.sort = self.order.first().and_then(|(name, desc)| {
            self.vis_cols.iter().position(|c| &c.name == name).map(|i| (i, if *desc { SortDir::Desc } else { SortDir::Asc }))
        });
        self.text_cache = None;
    }

    fn clear_edits(&mut self) {
        self.edits = Edits::default();
        self.confirm_delete = None;
        self.form_row = None;
        self.cell_key = None;
        self.text_cache = None;
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
                        self.clear_edits();
                        self.rebuild_visible();
                        if reset && !self.data.columns.is_empty() {
                            self.grid.reset();
                            self.rebuild_visible();
                        }
                        let flags = std::mem::take(&mut self.debug_flags);
                        if flags.iter().any(|f| f == "hide") && let Some(c) = self.columns.get(1) {
                            self.hidden.insert(c.name.clone());
                            self.rebuild_visible();
                        }
                        if flags.iter().any(|f| f == "filter") && let (Some(a), Some(b)) = (self.columns.get(1).cloned(), self.columns.first().cloned()) {
                            let mut first = Item::cond(a.name, Op::Contains, "1");
                            first.join = Join::Or;
                            self.fs.push(first);
                            let group = vec![Item::cond("city", Op::Eq, "Milano"), Item::cond("credit", Op::Gt, "400")];
                            self.fs.push(Item { enabled: true, join: Join::And, node: Node::Group(group) });
                            self.fs.select(vec![0]);
                            self.fs.sorts.push(Sort { col: b.name, desc: true });
                            self.apply_filter(env);
                        }
                        if flags.iter().any(|f| f == "picker")
                            && let Some(i) = self.vis_cols.iter().position(|c| DateKind::of(c.kind).is_some())
                        {
                            self.grid.open_date_picker(2, i as u32);
                        }
                        if flags.iter().any(|f| f == "edit") && !self.data.rows.is_empty() && self.columns.len() > 1 {
                            self.set_cell(0, 1, Some("Modificato (in sospeso)".into()));
                            self.edits.inserted.push(vec![None; self.columns.len()]);
                            self.grid.select_cell(0, 1);
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
                        env.actions.push(Action::Notice(format!("{n} record salvati"), false));
                        self.reload(env, false);
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        env.actions.push(Action::Notice(format!("Modifiche non salvate: {e}"), true));
                    }
                }
            }
            _ => {}
        }
    }

    fn editable(&self) -> bool {
        self.obj.kind == ObjectKind::Table && self.info.is_some() && !self.loading && !self.saving
    }

    fn row_count(&self) -> usize {
        self.data.rows.len() + self.edits.inserted.len()
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
        if changes.is_empty() || self.saving {
            return;
        }
        self.saving = true;
        self.confirm_delete = None;
        let (tab, obj) = (self.id, self.obj.clone());
        env.backend.with_driver(&cfg, &self.obj.db, move |d| async move {
            let res = match d {
                Ok(d) => d.apply_changes(obj.schema.as_deref(), &obj.name, &changes).await,
                Err(e) => Err(e),
            };
            Msg::Applied { tab, res }
        });
    }

    /// Sets a cell by result-set column.
    fn set_cell(&mut self, row: usize, dc: usize, value: Option<String>) {
        let v = value.map_or(Value::Null, Value::Text);
        let n = self.data.rows.len();
        if row >= n {
            if let Some(cell) = self.edits.inserted.get_mut(row - n).and_then(|r| r.get_mut(dc)) {
                *cell = Some(v);
            }
        } else {
            let original = self.data.rows.get(row).and_then(|r| r.get(dc));
            let same = original.is_some_and(|o| *o == v || (!o.is_null() && !v.is_null() && o.to_string() == v.to_string()));
            if same {
                self.edits.cells.remove(&(row, dc));
            } else {
                self.edits.cells.insert((row, dc), v);
            }
        }
        self.text_cache = None;
    }

    fn apply_filter(&mut self, env: &mut Env<'_>) {
        if !self.edits.is_empty() {
            env.actions.push(Action::Notice("Salva o annulla le modifiche prima di filtrare o ordinare".into(), true));
            return;
        }
        let d = env.dialect(&self.obj.conn);
        self.applied_where = self.fs.where_sql(d, &self.columns);
        self.order = self.fs.order();
        self.page = 0;
        self.rebuild_visible();
        self.reload(env, false);
    }

    fn on_grid_event(&mut self, ev: GridEvent, env: &mut Env<'_>) {
        let n = self.data.rows.len();
        let dc = |c: usize| self.col_map.get(c).copied();
        match ev {
            GridEvent::CellEdited { row, col, value } => {
                if let Some(c) = dc(col) {
                    self.set_cell(row, c, value);
                }
            }
            GridEvent::InsertRow => {
                self.edits.inserted.push(vec![None; self.columns.len()]);
                self.grid.select_cell((n + self.edits.inserted.len() - 1) as u32, 0);
                self.text_cache = None;
            }
            GridEvent::DeleteRows(rows) => {
                let mut ins: Vec<usize> = rows.iter().filter(|r| **r >= n).map(|r| r - n).collect();
                ins.sort_unstable_by(|a, b| b.cmp(a));
                for i in ins {
                    if i < self.edits.inserted.len() {
                        self.edits.inserted.remove(i);
                    }
                }
                let existing: Vec<usize> = rows.into_iter().filter(|r| *r < n).collect();
                for r in &existing {
                    if !self.edits.deleted.remove(r) {
                        self.edits.deleted.insert(*r);
                    }
                }
                if self.autocommit && !self.edits.deleted.is_empty() {
                    self.confirm_delete = Some(self.edits.deleted.len());
                }
                self.text_cache = None;
            }
            GridEvent::SortRequested { col, dir } => {
                let Some(name) = dc(col).and_then(|c| self.columns.get(c)).map(|c| c.name.clone()) else { return };
                self.fs.sorts = dir.map(|d| vec![Sort { col: name, desc: d == SortDir::Desc }]).unwrap_or_default();
                self.apply_filter(env);
            }
            GridEvent::FilterByValue { col, value } => {
                let Some(name) = dc(col).and_then(|c| self.columns.get(c)).map(|c| c.name.clone()) else { return };
                self.fs.raw_mode = false;
                let op = if value.is_none() { Op::IsNull } else { Op::Eq };
                self.fs.push(Item::cond(name, op, value.unwrap_or_default()));
                self.show_filter = true;
                self.apply_filter(env);
            }
            GridEvent::Activated { .. } | GridEvent::SelectionChanged => {}
        }
    }

    /// Auto-commit: save the pending record once the cursor has left it.
    fn autocommit_tick(&mut self, env: &mut Env<'_>) {
        if !self.autocommit || self.saving || self.loading || self.grid.is_editing() || self.confirm_delete.is_some() {
            return;
        }
        let n = self.data.rows.len();
        let mut dirty: BTreeSet<usize> = self.edits.cells.keys().map(|k| k.0).collect();
        dirty.extend((0..self.edits.inserted.len()).map(|i| n + i));
        if dirty.is_empty() {
            return;
        }
        let current = match self.view {
            View::Form => self.form_row.unwrap_or(self.grid.sel.active.0 as usize),
            _ => self.grid.sel.active.0 as usize,
        };
        if !dirty.contains(&current) {
            self.apply(env);
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

    fn model(&self, editable: bool) -> TableModel<'_> {
        TableModel { rs: &self.data, columns: &self.vis_cols, col_map: &self.col_map, edits: &self.edits, editable }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        let editable = self.editable();
        let dirty = !self.edits.is_empty();
        let mut reload = false;
        let is_table = self.obj.kind == ObjectKind::Table;

        // ----- toolbar -----
        ui.horizontal_wrapped(|ui| {
            if is_table {
                if self.autocommit {
                    if icons::button(ui, Db::Transaction, "Inizia transazione").on_hover_text("Le modifiche restano in sospeso finché non fai Commit (o Rollback)").clicked() {
                        self.autocommit = false;
                    }
                } else {
                    ui.label(RichText::new("● Transazione").color(egui::Color32::from_rgb(0xD9, 0x7A, 0x06)).strong());
                    if icons::button_ex(ui, Db::Commit, "Commit", false, dirty && editable).clicked() {
                        self.apply(env);
                        self.autocommit = true;
                    }
                    if icons::button(ui, Db::Rollback, "Rollback").clicked() {
                        self.clear_edits();
                        self.autocommit = true;
                    }
                }
                ui.separator();
            }
            if icons::button_ex(ui, Db::CellEditor, "Editor cella", self.show_cell_editor, true).clicked() {
                self.show_cell_editor = !self.show_cell_editor;
            }
            let fs_label = if self.applied_where.is_some() || !self.order.is_empty() { "Filtro e ordinamento ●" } else { "Filtro e ordinamento" };
            if icons::button_ex(ui, G::Filter, fs_label, self.show_filter, true).clicked() {
                self.show_filter = !self.show_filter;
            }
            let mut changed = false;
            let cols_label = format!("Colonne{}", if self.hidden.is_empty() { String::new() } else { format!(" ({} nascoste)", self.hidden.len()) });
            let cols_btn = icons::button(ui, Db::Columns, &cols_label);
            egui::Popup::menu(&cols_btn).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_max_height(400.0);
                if ui.button("Mostra tutte").clicked() {
                    self.hidden.clear();
                    changed = true;
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for c in &self.columns {
                        let mut shown = !self.hidden.contains(&c.name);
                        if ui.checkbox(&mut shown, &c.name).changed() {
                            if shown {
                                self.hidden.remove(&c.name);
                            } else if self.hidden.len() + 1 < self.columns.len() {
                                self.hidden.insert(c.name.clone());
                            }
                            changed = true;
                        }
                    }
                });
            });
            if changed {
                self.rebuild_visible();
            }
            ui.separator();
            if is_table && icons::button(ui, Db::Import, "Importa").clicked() {
                env.actions.push(Action::Import(self.obj.clone()));
            }
            if icons::button(ui, Db::Export, "Esporta").clicked() {
                env.actions.push(Action::Export(ExportSource::Table(self.obj.clone())));
            }
            if is_table && icons::button(ui, Db::Design, "Progetta").clicked() {
                env.actions.push(Action::DesignTable(self.obj.clone()));
            }
            if self.loading || self.saving {
                ui.spinner();
            }
        });

        if self.show_filter {
            let d = env.dialect(&self.obj.conn);
            if self.fs.ui(ui, &self.columns, d) {
                self.apply_filter(env);
            }
        }
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C), e);
        }
        if let Some(n) = self.confirm_delete {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Eliminare {n} record dal database?")).strong());
                if ui.button(RichText::new("Elimina").color(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C))).clicked() {
                    self.apply(env);
                }
                if ui.button("Annulla").clicked() {
                    self.edits.deleted.clear();
                    self.confirm_delete = None;
                }
            });
        }
        if self.info.as_ref().is_some_and(|i| i.primary_key().is_empty()) && is_table {
            ui.label(RichText::new("⚠ La tabella non ha chiave primaria: le modifiche usano tutti i valori della riga per identificarla.").small().weak());
        }

        self.bottom_bar(ui, env, editable, dirty, &mut reload);
        if self.show_cell_editor {
            self.cell_editor(ui, env, editable);
        }

        match self.view {
            View::Grid => {
                let events = {
                    let model = TableModel { rs: &self.data, columns: &self.vis_cols, col_map: &self.col_map, edits: &self.edits, editable };
                    self.grid.show(ui, &model)
                };
                for ev in events {
                    self.on_grid_event(ev, env);
                }
            }
            View::Form => self.form_view(ui, editable),
            View::Text => self.text_view(ui),
        }
        self.autocommit_tick(env);
        if reload && !self.loading {
            self.reload(env, false);
        }
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>, editable: bool, dirty: bool, reload: &mut bool) {
        let pages = self.total.map(|t| t.div_ceil(self.page_size).max(1));
        egui::Panel::bottom(egui::Id::unique(("pager", self.id))).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if icons::small(ui, G::Plus, "Aggiungi record", editable).clicked() {
                    self.view = if self.view == View::Text { View::Grid } else { self.view };
                    self.on_grid_event(GridEvent::InsertRow, env);
                    self.form_row = Some(self.row_count() - 1);
                }
                if icons::small(ui, G::Minus, "Elimina record selezionati", editable).clicked() {
                    let rows = match self.view {
                        View::Form => self.form_row.into_iter().collect(),
                        _ => self.grid.sel.rows(self.row_count() as u32, self.vis_cols.len() as u32),
                    };
                    self.on_grid_event(GridEvent::DeleteRows(rows), env);
                }
                if icons::small(ui, G::Check, "Salva modifiche", dirty && editable).clicked() {
                    self.apply(env);
                }
                if icons::small(ui, G::Close, "Annulla modifiche", dirty).clicked() {
                    self.clear_edits();
                }
                if icons::small(ui, Db::Refresh, "Aggiorna", !self.loading).clicked() {
                    *reload = true;
                }
                ui.separator();
                let cur = match self.view {
                    View::Form => self.form_row.unwrap_or(0),
                    _ => self.grid.sel.active.0 as usize,
                };
                let abs = self.page * self.page_size + cur as u64 + 1;
                let total = self.total.map(|t| format!(" di {t}")).unwrap_or_default();
                ui.label(RichText::new(if self.row_count() == 0 { "Nessun record".to_string() } else { format!("Record {abs}{total}") }).small());
                ui.separator();
                let before = self.page_size;
                egui::ComboBox::from_id_salt(("ps", self.id)).selected_text(format!("{} righe", self.page_size)).show_ui(ui, |ui| {
                    for n in [100, 500, 1000, 5000, 10000, 50000] {
                        ui.selectable_value(&mut self.page_size, n, n.to_string());
                    }
                });
                if before != self.page_size && !dirty {
                    self.page = 0;
                    *reload = true;
                }
                let last = pages.map_or(u64::MAX, |p| p - 1);
                if ui.add_enabled(!dirty && self.page > 0, egui::Button::new("⏮").small()).clicked() {
                    self.page = 0;
                    *reload = true;
                }
                if ui.add_enabled(!dirty && self.page > 0, egui::Button::new("◀").small()).clicked() {
                    self.page -= 1;
                    *reload = true;
                }
                ui.label(format!("Pagina {}{}", self.page + 1, pages.map(|p| format!(" / {p}")).unwrap_or_default()));
                if ui.add_enabled(!dirty && self.page < last && self.data.rows.len() as u64 == self.page_size, egui::Button::new("▶").small()).clicked() {
                    self.page += 1;
                    *reload = true;
                }
                if ui.add_enabled(!dirty && self.page < last && pages.is_some(), egui::Button::new("⏭").small()).clicked() {
                    self.page = last;
                    *reload = true;
                }
                ui.separator();
                for (v, icon, tip) in [(View::Grid, Db::GridView, "Vista griglia"), (View::Form, Db::FormView, "Vista modulo"), (View::Text, Db::TextView, "Vista testo")] {
                    if icons::button_ex(ui, icon, "", self.view == v, true).on_hover_text(tip).clicked() {
                        self.view = v;
                        self.form_row = None;
                    }
                }
            });
        });
    }

    /// The selected cell as (row, result-set column).
    fn current_cell(&self) -> Option<(usize, usize)> {
        let (r, c) = self.grid.sel.active;
        let row = match self.view {
            View::Form => self.form_row.unwrap_or(r as usize),
            _ => r as usize,
        };
        let dc = *self.col_map.get(c as usize)?;
        (row < self.row_count()).then_some((row, dc))
    }

    fn cell_editor(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>, editable: bool) {
        let key = self.current_cell();
        let value = key.and_then(|(r, c)| self.model(editable).value(r, c));
        if key != self.cell_key {
            self.cell_key = key;
            self.cell_picker = None;
            self.cell_buf = match &value {
                Some(Value::Bytes(b)) => hex_dump(b),
                Some(v) => value_text(v),
                None => String::new(),
            };
        }
        egui::Panel::right(egui::Id::unique(("celled", self.id))).resizable(true).default_size(320.0).min_size(220.0).show(ui, |ui| {
            let Some((row, dc)) = key else {
                ui.label(RichText::new("Seleziona una cella").weak());
                return;
            };
            let col = self.columns.get(dc).cloned().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.strong(&col.name);
                ui.label(RichText::new(&col.type_name).small().weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("riga {}", row + 1)).small().weak());
                });
            });
            let binary = matches!(value, Some(Value::Bytes(_)));
            let is_null = matches!(value, None | Some(Value::Null));
            if let Some(Value::Bytes(b)) = &value {
                ui.label(RichText::new(format!("Dati binari: {} byte", b.len())).small());
            } else if is_null {
                ui.label(RichText::new("NULL").small().weak().italics());
            }
            let can_edit = editable && !binary;
            ui.horizontal(|ui| {
                if icons::button_ex(ui, G::Check, "Applica", false, can_edit).clicked() {
                    self.set_cell(row, dc, Some(self.cell_buf.clone()));
                    self.cell_key = None;
                }
                if ui.add_enabled(editable, egui::Button::new("NULL")).on_hover_text("Imposta a NULL").clicked() {
                    self.set_cell(row, dc, None);
                    self.cell_key = None;
                }
                let json_like = col.kind == ColumnKind::Json || self.cell_buf.trim_start().starts_with(['{', '[']);
                if json_like && ui.button("{ } Formatta JSON").clicked() {
                    match serde_json::from_str::<serde_json::Value>(&self.cell_buf).and_then(|v| serde_json::to_string_pretty(&v)) {
                        Ok(p) => self.cell_buf = p,
                        Err(e) => env.actions.push(Action::Notice(format!("JSON non valido: {e}"), true)),
                    }
                }
                if icons::small(ui, G::Copy, "Copia", true).clicked() {
                    ui.ctx().copy_text(self.cell_buf.clone());
                }
            });
            if editable && let Some(kind) = DateKind::of(col.kind) {
                let buf = self.cell_buf.clone();
                let st = self.cell_picker.get_or_insert_with(|| PickerState::from_text(&buf, kind));
                let outcome = egui::Frame::group(ui.style()).show(ui, |ui| st.ui(ui, true)).inner;
                match outcome {
                    Some(PickerOutcome::Pick(t)) => {
                        self.cell_buf = t.clone();
                        self.set_cell(row, dc, Some(t));
                        self.cell_key = None;
                    }
                    Some(PickerOutcome::Null) => {
                        self.set_cell(row, dc, None);
                        self.cell_key = None;
                    }
                    Some(PickerOutcome::Cancel) => self.cell_picker = None,
                    None => {}
                }
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut self.cell_buf).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(20).interactive(true));
            });
        });
    }

    fn form_view(&mut self, ui: &mut egui::Ui, editable: bool) {
        let total = self.row_count();
        if total == 0 {
            ui.label(RichText::new("Nessun record").weak());
            return;
        }
        let row = self.form_row.unwrap_or(self.grid.sel.active.0 as usize).min(total - 1);
        ui.horizontal(|ui| {
            if ui.add_enabled(row > 0, egui::Button::new("◀ Precedente")).clicked() {
                self.form_row = Some(row - 1);
            }
            ui.label(format!("Record {} di {}", row + 1, total));
            if ui.add_enabled(row + 1 < total, egui::Button::new("Successivo ▶")).clicked() {
                self.form_row = Some(row + 1);
            }
        });
        let row = self.form_row.unwrap_or(row).min(total - 1);
        if self.form_row != Some(row) || self.form_bufs.len() != self.col_map.len() {
            let m = self.model(editable);
            self.form_bufs = self.col_map.iter().map(|&dc| m.value(row, dc).map(|v| value_text(&v)).unwrap_or_default()).collect();
            self.form_row = Some(row);
            self.grid.select_cell(row as u32, self.grid.sel.active.1);
        }
        ui.separator();
        let mut commits: Vec<(usize, Option<String>)> = Vec::new();
        let mut picker_open: Option<(usize, egui::Rect)> = None;
        // Label (~120), type, calendar and NULL buttons (~230) take the rest.
        let width = (ui.available_width() - 350.0).clamp(120.0, 640.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new(("form", self.id)).num_columns(3).striped(true).spacing([12.0, 6.0]).show(ui, |ui| {
                let m = TableModel { rs: &self.data, columns: &self.vis_cols, col_map: &self.col_map, edits: &self.edits, editable };
                for (i, &dc) in self.col_map.iter().enumerate() {
                    let Some(col) = self.columns.get(dc) else { continue };
                    ui.horizontal(|ui| {
                        if col.primary_key {
                            icons::show(ui, Db::Key, 14.0);
                        }
                        ui.strong(&col.name);
                    });
                    let current = m.value(row, dc);
                    let null = matches!(current, None | Some(Value::Null));
                    let binary = matches!(current, Some(Value::Bytes(_)));
                    let Some(buf) = self.form_bufs.get_mut(i) else { continue };
                    let multiline = buf.contains('\n') || buf.len() > 80;
                    let hint = if null { "(NULL)" } else { "" };
                    let edit = if multiline { egui::TextEdit::multiline(buf).desired_rows(3) } else { egui::TextEdit::singleline(buf) }.hint_text(hint);
                    let size = egui::vec2(width, if multiline { 64.0 } else { ui.spacing().interact_size.y });
                    let r = ui.add_enabled_ui(editable && !binary, |ui| ui.add_sized(size, edit)).inner;
                    if r.lost_focus() && r.changed() || (r.lost_focus() && current.as_ref().map(value_text).as_deref() != Some(buf.as_str()) && !(null && buf.is_empty())) {
                        commits.push((dc, Some(buf.clone())));
                    }
                    ui.horizontal(|ui| {
                        if editable && DateKind::of(col.kind).is_some() && icons::small(ui, G::Calendar, "Scegli data/ora", true).clicked() {
                            picker_open = Some((i, r.rect));
                        }
                        ui.label(RichText::new(&col.type_name).small().weak());
                        if editable && !null && ui.small_button("NULL").clicked() {
                            commits.push((dc, None));
                            buf.clear();
                        }
                    });
                    ui.end_row();
                }
            });
        });
        if picker_open.is_some() {
            self.form_picker = picker_open;
        }
        if let Some((i, anchor)) = self.form_picker {
            let dc = self.col_map.get(i).copied();
            let kind = dc.and_then(|dc| self.columns.get(dc)).and_then(|c| DateKind::of(c.kind));
            match (dc, kind) {
                (Some(dc), Some(kind)) => {
                    let current = self.form_bufs.get(i).cloned().unwrap_or_default();
                    match datetime::popup(ui.ctx(), egui::Id::unique(("form-picker", self.id, i)), anchor, kind, &current, true) {
                        Some(PickerOutcome::Pick(t)) => {
                            if let Some(b) = self.form_bufs.get_mut(i) {
                                *b = t.clone();
                            }
                            commits.push((dc, Some(t)));
                            self.form_picker = None;
                        }
                        Some(PickerOutcome::Null) => {
                            if let Some(b) = self.form_bufs.get_mut(i) {
                                b.clear();
                            }
                            commits.push((dc, None));
                            self.form_picker = None;
                        }
                        Some(PickerOutcome::Cancel) => self.form_picker = None,
                        None => {}
                    }
                }
                _ => self.form_picker = None,
            }
        }
        for (dc, v) in commits {
            self.set_cell(row, dc, v);
        }
    }

    fn text_view(&mut self, ui: &mut egui::Ui) {
        if self.text_cache.is_none() {
            let m = self.model(false);
            let rows = self.row_count().min(5000);
            let cells: Vec<Vec<String>> = (0..rows)
                .map(|r| self.col_map.iter().map(|&dc| m.value(r, dc).map_or_else(|| "(NULL)".into(), |v| if v.is_null() { "(NULL)".into() } else { v.to_string().replace(['\n', '\r', '\t'], " ") })).collect())
                .collect();
            let widths: Vec<usize> = self
                .vis_cols
                .iter()
                .enumerate()
                .map(|(i, c)| cells.iter().map(|r| r.get(i).map_or(0, |s| s.chars().count())).max().unwrap_or(0).max(c.name.chars().count()).min(60))
                .collect();
            let fit = |s: &str, w: usize| {
                let n = s.chars().count();
                if n > w { format!("{}…", s.chars().take(w.saturating_sub(1)).collect::<String>()) } else { format!("{s}{}", " ".repeat(w - n)) }
            };
            let mut out = String::new();
            out.push_str(&self.vis_cols.iter().zip(&widths).map(|(c, w)| fit(&c.name, *w)).collect::<Vec<_>>().join(" | "));
            out.push('\n');
            out.push_str(&widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("-+-"));
            out.push('\n');
            for r in &cells {
                out.push_str(&r.iter().zip(&widths).map(|(s, w)| fit(s, *w)).collect::<Vec<_>>().join(" | "));
                out.push('\n');
            }
            self.text_cache = Some(out);
        }
        if let Some(text) = &self.text_cache {
            let mut t = text.as_str();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                ui.add(egui::TextEdit::multiline(&mut t).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY));
            });
        }
    }
}

fn hex_dump(b: &[u8]) -> String {
    let mut out = String::new();
    for (i, chunk) in b.chunks(16).take(4096).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|x| format!("{x:02X}")).collect();
        let ascii: String = chunk.iter().map(|&x| if (0x20..0x7f).contains(&x) { x as char } else { '.' }).collect();
        out.push_str(&format!("{:08X}  {:<47}  {ascii}\n", i * 16, hex.join(" ")));
    }
    if b.len() > 16 * 4096 {
        out.push_str("…\n");
    }
    out
}
