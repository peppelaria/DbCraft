//! Visual query builder: drop tables on a canvas, tick columns, drag a column onto another
//! table's column to join them, then set conditions, sorting, grouping and limits. The SQL is
//! regenerated from the model on every change and handed back to the query editor.

use crate::icons::{self, Db, G};
use db_core::{ColumnInfo, DbResult, Dialect, ForeignKeyInfo, ObjectKind, SchemaObject, TableInfo};
use dbgrid::GridColumn;
use egui::{Align2, Color32, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::models::kind_of;
use crate::tabs::filter::{FilterSort, quote_path};
use crate::i18n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinKind {
    const ALL: [JoinKind; 4] = [JoinKind::Inner, JoinKind::Left, JoinKind::Right, JoinKind::Full];
    fn sql(self) -> &'static str {
        match self {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
            JoinKind::Right => "RIGHT JOIN",
            JoinKind::Full => "FULL OUTER JOIN",
        }
    }
    fn short(self) -> &'static str {
        match self {
            JoinKind::Inner => "=",
            JoinKind::Left => "⟕",
            JoinKind::Right => "⟖",
            JoinKind::Full => "⟗",
        }
    }
    fn label(self) -> &'static str {
        match self {
            JoinKind::Inner => "INNER JOIN (solo righe corrispondenti)",
            JoinKind::Left => "LEFT JOIN (tutte le righe a sinistra)",
            JoinKind::Right => "RIGHT JOIN (tutte le righe a destra)",
            JoinKind::Full => "FULL OUTER JOIN (tutte le righe)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agg {
    None,
    Count,
    CountDistinct,
    Sum,
    Avg,
    Min,
    Max,
}

impl Agg {
    const ALL: [Agg; 7] = [Agg::None, Agg::Count, Agg::CountDistinct, Agg::Sum, Agg::Avg, Agg::Min, Agg::Max];
    fn label(self) -> &'static str {
        match self {
            Agg::None => "—",
            Agg::Count => "COUNT",
            Agg::CountDistinct => "COUNT DISTINCT",
            Agg::Sum => "SUM",
            Agg::Avg => "AVG",
            Agg::Min => "MIN",
            Agg::Max => "MAX",
        }
    }
    fn wrap(self, e: &str) -> String {
        match self {
            Agg::None => e.to_string(),
            Agg::CountDistinct => format!("COUNT(DISTINCT {e})"),
            other => format!("{}({e})", other.label()),
        }
    }
}

pub struct BTable {
    pub name: String,
    pub alias: String,
    pub columns: Vec<ColumnInfo>,
    pub fks: Vec<ForeignKeyInfo>,
    pub loading: bool,
    pub error: Option<String>,
    /// Canvas position of the top-left corner.
    pub pos: Pos2,
    scroll: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BJoin {
    /// (table index, column)
    pub a: (usize, String),
    pub b: (usize, String),
    pub kind: JoinKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BField {
    pub table: usize,
    /// Column name, or `*`.
    pub col: String,
    pub alias: String,
    pub agg: Agg,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Fields,
    Conditions,
    Sql,
}

/// What the builder needs from the outside (loading table lists and definitions).
pub enum BuilderRequest {
    Describe(String),
}

pub struct QueryBuilder {
    pub dialect: Dialect,
    pub schema: Option<String>,
    pub available: Option<Result<Vec<String>, String>>,
    search: String,
    pub tables: Vec<BTable>,
    pub joins: Vec<BJoin>,
    pub fields: Vec<BField>,
    pub filter: FilterSort,
    pub distinct: bool,
    pub limit: String,
    pane: Pane,
    pan: Vec2,
    drag_join: Option<(usize, String)>,
    pub requests: Vec<BuilderRequest>,
}

const TABLE_W: f32 = 210.0;
/// The builder's blue (table headers, joins) and the green of a valid drop.
const ACCENT: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xD0);
const OK: Color32 = Color32::from_rgb(0x1E, 0x9E, 0x5A);

/// What is being dragged, next to the pointer and above everything: a table name, or a join
/// `a.col = b.col` (`?` while there is no target yet).
fn ghost_chip(ctx: &egui::Context, p: Pos2, text: &str, to: Option<&str>) {
    let fg = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("qb-ghost")));
    let label = match to {
        Some(t) => format!("{text} = {t}"),
        None => text.to_string(),
    };
    let galley = fg.layout_no_wrap(label, FontId::proportional(12.5), Color32::WHITE);
    let r = Rect::from_min_size(p + vec2(14.0, 12.0), galley.size() + vec2(16.0, 8.0));
    let color = if to.is_some_and(|t| t != "?") { OK } else { ACCENT };
    fg.rect_filled(r.translate(vec2(2.0, 3.0)), 6.0, Color32::from_black_alpha(40));
    fg.rect_filled(r, 6.0, color.gamma_multiply(0.85));
    fg.galley(r.min + vec2(8.0, 4.0), galley, Color32::WHITE);
}
const HEAD_H: f32 = 24.0;
const ROW_H: f32 = 20.0;
const MAX_ROWS: usize = 12;

impl QueryBuilder {
    pub fn new(dialect: Dialect, schema: Option<String>) -> Self {
        QueryBuilder {
            dialect,
            schema,
            available: None,
            search: String::new(),
            tables: Vec::new(),
            joins: Vec::new(),
            fields: Vec::new(),
            filter: FilterSort::for_builder(),
            distinct: false,
            limit: String::new(),
            pane: Pane::Fields,
            pan: Vec2::ZERO,
            drag_join: None,
            requests: Vec::new(),
        }
    }

    pub fn on_tables(&mut self, res: DbResult<Vec<SchemaObject>>) {
        self.available = Some(
            res.map(|v| v.into_iter().filter(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View)).map(|o| o.name).collect()).map_err(|e| e.to_string()),
        );
    }

    pub fn on_table_info(&mut self, name: &str, res: DbResult<TableInfo>) {
        let mut loaded = Vec::new();
        for (i, t) in self.tables.iter_mut().enumerate().filter(|(_, t)| t.name == name && t.loading) {
            t.loading = false;
            match &res {
                Ok(info) => {
                    t.columns = info.columns.clone();
                    t.fks = info.foreign_keys.clone();
                    loaded.push(i);
                }
                Err(e) => t.error = Some(e.to_string()),
            }
        }
        for i in loaded {
            self.auto_join(i);
        }
    }

    pub fn add_table(&mut self, name: &str, pos: Option<Pos2>) {
        let base = name.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
        let base = if base.is_empty() { "t".to_string() } else { base };
        let mut alias = base.clone();
        let mut n = 2;
        while self.tables.iter().any(|t| t.alias == alias) {
            alias = format!("{base}{n}");
            n += 1;
        }
        let k = self.tables.len();
        let pos = pos.unwrap_or_else(|| pos2(20.0 + (k % 4) as f32 * (TABLE_W + 50.0), 20.0 + (k / 4) as f32 * 300.0 + (k % 2) as f32 * 30.0));
        self.tables.push(BTable { name: name.to_string(), alias, columns: Vec::new(), fks: Vec::new(), loading: true, error: None, pos, scroll: 0.0 });
        self.requests.push(BuilderRequest::Describe(name.to_string()));
    }

    /// Joins the new table `i` to the others through declared foreign keys (either direction).
    fn auto_join(&mut self, i: usize) {
        let mut new = Vec::new();
        for (j, other) in self.tables.iter().enumerate().filter(|(j, _)| *j != i) {
            let Some(me) = self.tables.get(i) else { return };
            for fk in &me.fks {
                if fk.ref_table == other.name
                    && let (Some(c), Some(r)) = (fk.columns.first(), fk.ref_columns.first())
                {
                    new.push(BJoin { a: (j, r.clone()), b: (i, c.clone()), kind: JoinKind::Inner });
                }
            }
            for fk in &other.fks {
                if fk.ref_table == me.name
                    && let (Some(c), Some(r)) = (fk.columns.first(), fk.ref_columns.first())
                {
                    new.push(BJoin { a: (j, c.clone()), b: (i, r.clone()), kind: JoinKind::Inner });
                }
            }
        }
        for j in new {
            if !self.joins.iter().any(|x| (x.a == j.a && x.b == j.b) || (x.a == j.b && x.b == j.a)) {
                self.joins.push(j);
            }
        }
    }

    fn remove_table(&mut self, i: usize) {
        if i >= self.tables.len() {
            return;
        }
        self.tables.remove(i);
        let fix = |t: usize| if t > i { t - 1 } else { t };
        self.joins.retain(|j| j.a.0 != i && j.b.0 != i);
        for j in &mut self.joins {
            j.a.0 = fix(j.a.0);
            j.b.0 = fix(j.b.0);
        }
        self.fields.retain(|f| f.table != i);
        for f in &mut self.fields {
            f.table = fix(f.table);
        }
    }

    /// `[agg:]alias.column`, for scripted screenshots.
    pub fn debug_field(&mut self, spec: &str) {
        if spec == "sql" {
            self.pane = Pane::Sql;
            return;
        }
        let (agg, path) = match spec.split_once(':') {
            Some(("sum", p)) => (Agg::Sum, p),
            Some(("count", p)) => (Agg::Count, p),
            Some((_, p)) => (Agg::None, p),
            None => (Agg::None, spec),
        };
        let Some((alias, col)) = path.split_once('.') else { return };
        if let Some(t) = self.tables.iter().position(|t| t.alias == alias) {
            self.fields.push(BField { table: t, col: col.to_string(), alias: String::new(), agg });
        }
    }

    fn toggle_field(&mut self, table: usize, col: &str) {
        match self.fields.iter().position(|f| f.table == table && f.col == col) {
            Some(p) => {
                self.fields.remove(p);
            }
            None => self.fields.push(BField { table, col: col.to_string(), alias: String::new(), agg: Agg::None }),
        }
    }

    fn table_ref(&self, t: &BTable) -> String {
        let d = self.dialect;
        let name = d.qualified(self.schema.as_deref(), &t.name);
        if t.alias == t.name { name } else { format!("{name} {}", d.quote(&t.alias)) }
    }

    fn col_ref(&self, table: usize, col: &str) -> String {
        let Some(t) = self.tables.get(table) else { return col.to_string() };
        if col == "*" { format!("{}.*", self.dialect.quote(&t.alias)) } else { format!("{}.{}", self.dialect.quote(&t.alias), self.dialect.quote(col)) }
    }

    /// Columns offered to the condition editor, as `alias.column`.
    fn qualified_columns(&self) -> Vec<GridColumn> {
        self.tables
            .iter()
            .flat_map(|t| {
                t.columns.iter().map(|c| GridColumn { name: format!("{}.{}", t.alias, c.name), type_name: c.data_type.clone(), kind: kind_of(&c.data_type), primary_key: c.primary_key, width: None })
            })
            .collect()
    }

    pub fn sql(&self) -> String {
        let d = self.dialect;
        if self.tables.is_empty() {
            return String::new();
        }
        let limit = self.limit.trim().parse::<u64>().ok();
        let mut s = String::from("SELECT ");
        if self.distinct {
            s.push_str("DISTINCT ");
        }
        if d == Dialect::SqlServer
            && let Some(n) = limit
        {
            s.push_str(&format!("TOP {n} "));
        }
        let cols: Vec<String> = self
            .fields
            .iter()
            .map(|f| {
                let e = f.agg.wrap(&self.col_ref(f.table, &f.col));
                if f.alias.trim().is_empty() { e } else { format!("{e} AS {}", d.quote(f.alias.trim())) }
            })
            .collect();
        s.push_str(&if cols.is_empty() { "*".to_string() } else { cols.join(",\n       ") });

        // FROM: the first table, then each table joined to one already placed; leftovers are cross joined.
        s.push_str(&format!("\nFROM {}", self.table_ref(&self.tables[0])));
        let mut placed = vec![0usize];
        let mut used = vec![false; self.joins.len()];
        loop {
            let mut progressed = false;
            for t in 1..self.tables.len() {
                if placed.contains(&t) {
                    continue;
                }
                let conds: Vec<(usize, &BJoin)> = self
                    .joins
                    .iter()
                    .enumerate()
                    .filter(|(k, j)| !used[*k] && ((j.a.0 == t && placed.contains(&j.b.0)) || (j.b.0 == t && placed.contains(&j.a.0))))
                    .collect();
                let Some(&(_, first)) = conds.first() else { continue };
                // The join kind reads from the already-placed side towards `t`.
                let kind = if first.b.0 == t {
                    first.kind
                } else {
                    match first.kind {
                        JoinKind::Left => JoinKind::Right,
                        JoinKind::Right => JoinKind::Left,
                        k => k,
                    }
                };
                let on: Vec<String> = conds.iter().map(|(_, j)| format!("{} = {}", self.col_ref(j.a.0, &j.a.1), self.col_ref(j.b.0, &j.b.1))).collect();
                s.push_str(&format!("\n{} {} ON {}", kind.sql(), self.table_ref(&self.tables[t]), on.join(" AND ")));
                for (k, _) in conds {
                    used[k] = true;
                }
                placed.push(t);
                progressed = true;
            }
            if !progressed {
                break;
            }
        }
        for t in 1..self.tables.len() {
            if !placed.contains(&t) {
                s.push_str(&format!("\nCROSS JOIN {}", self.table_ref(&self.tables[t])));
                placed.push(t);
            }
        }
        // Joins between tables placed earlier that were not used as ON clauses.
        let extra: Vec<String> =
            self.joins.iter().enumerate().filter(|(k, _)| !used[*k]).map(|(_, j)| format!("{} = {}", self.col_ref(j.a.0, &j.a.1), self.col_ref(j.b.0, &j.b.1))).collect();
        let cols = self.qualified_columns();
        let mut wheres: Vec<String> = extra;
        if let Some(w) = self.filter.where_sql(d, &cols) {
            wheres.push(if wheres.is_empty() { w } else { format!("({w})") });
        }
        if !wheres.is_empty() {
            s.push_str(&format!("\nWHERE {}", wheres.join(" AND ")));
        }
        if self.fields.iter().any(|f| f.agg != Agg::None) {
            let group: Vec<String> = self.fields.iter().filter(|f| f.agg == Agg::None && f.col != "*").map(|f| self.col_ref(f.table, &f.col)).collect();
            if !group.is_empty() {
                s.push_str(&format!("\nGROUP BY {}", group.join(", ")));
            }
        }
        let order: Vec<String> = self.filter.order().into_iter().map(|(c, desc)| format!("{}{}", quote_path(d, &c), if desc { " DESC" } else { "" })).collect();
        if !order.is_empty() {
            s.push_str(&format!("\nORDER BY {}", order.join(", ")));
        }
        if let Some(n) = limit
            && d != Dialect::SqlServer
        {
            s.push_str(&format!("\nLIMIT {n}"));
        }
        s
    }

    // ----- UI -----

    /// Returns `Some(true)` for OK (use the SQL) and `Some(false)` for Cancel.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<bool> {
        let mut out = None;
        ui.horizontal(|ui| {
            ui.heading(i18n::t("Query Builder"));
            ui.label(RichText::new(i18n::t("Trascina le tabelle sulla tela; trascina una colonna su un'altra colonna per unire le tabelle.")).small().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ok = egui::Button::new(RichText::new(i18n::t("OK — usa questa query")).color(Color32::WHITE).strong()).fill(Color32::from_rgb(0x1F, 0x6F, 0xD0));
                if ui.add_enabled(!self.tables.is_empty(), ok).clicked() {
                    out = Some(true);
                }
                if ui.button(i18n::t("Annulla")).clicked() {
                    out = Some(false);
                }
            });
        });
        ui.separator();

        egui::Panel::left("qb-tables").resizable(true).default_size(200.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                icons::show(ui, G::Search, 16.0);
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(i18n::t("Cerca tabella")).desired_width(f32::INFINITY));
            });
            let mut add = None;
            egui::ScrollArea::vertical().show(ui, |ui| match &self.available {
                None => {
                    ui.spinner();
                }
                Some(Err(e)) => {
                    ui.colored_label(Color32::RED, e);
                }
                Some(Ok(names)) => {
                    let q = self.search.to_lowercase();
                    for n in names.iter().filter(|n| q.is_empty() || n.to_lowercase().contains(&q)) {
                        let r = ui
                            .horizontal(|ui| {
                                icons::show(ui, Db::Table, 15.0);
                                ui.add(egui::Label::new(n.as_str()).selectable(false).sense(Sense::click_and_drag()))
                            })
                            .inner
                            .on_hover_text(i18n::t("Doppio clic o trascina sulla tela"));
                        r.dnd_set_drag_payload(n.clone());
                        if r.double_clicked() {
                            add = Some(n.clone());
                        }
                    }
                }
            });
            if let Some(n) = add {
                self.add_table(&n, None);
            }
        });

        egui::Panel::bottom("qb-bottom").resizable(true).default_size(260.0).min_size(140.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.pane, Pane::Fields, i18n::tf(format!("Campi ({})", self.fields.len())));
                ui.selectable_value(&mut self.pane, Pane::Conditions, i18n::t("Condizioni e ordinamento"));
                ui.selectable_value(&mut self.pane, Pane::Sql, i18n::t("SQL"));
                ui.separator();
                ui.checkbox(&mut self.distinct, i18n::t("DISTINCT"));
                ui.label(i18n::t("Limite"));
                ui.add(egui::TextEdit::singleline(&mut self.limit).desired_width(60.0).hint_text(i18n::t("tutte")));
            });
            ui.separator();
            match self.pane {
                Pane::Fields => self.fields_ui(ui),
                Pane::Conditions => {
                    let cols = self.qualified_columns();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let _ = self.filter.ui(ui, &cols, self.dialect);
                    });
                }
                Pane::Sql => {
                    let mut sql = self.sql();
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.add(egui::TextEdit::multiline(&mut sql).code_editor().desired_width(f32::INFINITY));
                    });
                }
            }
        });

        self.canvas(ui);
        out
    }

    fn fields_ui(&mut self, ui: &mut egui::Ui) {
        if self.fields.is_empty() {
            ui.label(RichText::new(i18n::t("Nessun campo selezionato: verranno restituite tutte le colonne (*). Spunta le colonne nelle tabelle.")).weak());
            return;
        }
        let mut remove = None;
        let mut swap = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("qb-fields").striped(true).num_columns(5).spacing([10.0, 4.0]).show(ui, |ui| {
                for h in ["Campo", "Funzione", "Alias", "", ""] {
                    ui.strong(i18n::t(h));
                }
                ui.end_row();
                let n = self.fields.len();
                for i in 0..n {
                    let label = self.fields.get(i).map(|f| format!("{}.{}", self.tables.get(f.table).map_or("?", |t| t.alias.as_str()), f.col)).unwrap_or_default();
                    let Some(f) = self.fields.get_mut(i) else { continue };
                    ui.label(RichText::new(label).monospace());
                    egui::ComboBox::from_id_salt(("agg", i)).selected_text(f.agg.label()).show_ui(ui, |ui| {
                        for a in Agg::ALL {
                            ui.selectable_value(&mut f.agg, a, a.label());
                        }
                    });
                    ui.add_sized([160.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut f.alias).hint_text(i18n::t("alias")));
                    ui.horizontal(|ui| {
                        if i > 0 && icons::small(ui, G::ChevronUp, "Sposta su", true).clicked() {
                            swap = Some(i);
                        }
                    });
                    if icons::small(ui, G::Delete, "Rimuovi", true).clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if self.fields.iter().any(|f| f.agg != Agg::None) {
                ui.label(RichText::new(i18n::t("Con funzioni di aggregazione, i campi senza funzione vengono raggruppati (GROUP BY) automaticamente.")).small().weak());
            }
        });
        if let Some(i) = swap {
            self.fields.swap(i, i - 1);
        }
        if let Some(i) = remove {
            self.fields.remove(i);
        }
    }

    fn table_rect(&self, t: &BTable, origin: Pos2) -> Rect {
        let rows = (t.columns.len() + 1).clamp(1, MAX_ROWS);
        Rect::from_min_size(origin + t.pos.to_vec2() + self.pan, vec2(TABLE_W, HEAD_H + rows as f32 * ROW_H + 4.0))
    }

    /// Screen rect of a column row (row 0 is `*`), if visible.
    fn row_rect(&self, t: &BTable, origin: Pos2, row: usize) -> Option<Rect> {
        let r = self.table_rect(t, origin);
        let y = r.top() + HEAD_H + row as f32 * ROW_H - t.scroll;
        (y >= r.top() + HEAD_H - 1.0 && y + ROW_H <= r.bottom() + 1.0).then(|| Rect::from_min_size(pos2(r.left(), y), vec2(TABLE_W, ROW_H)))
    }

    /// Anchor of a column on the side of its table that faces `toward`.
    fn anchor(&self, t: usize, col: &str, origin: Pos2, toward: f32) -> Option<Pos2> {
        let table = self.tables.get(t)?;
        let r = self.table_rect(table, origin);
        let row = table.columns.iter().position(|c| c.name == col)? + 1;
        let y = match self.row_rect(table, origin, row) {
            Some(rr) => rr.center().y,
            None => (r.top() + HEAD_H / 2.0).max(r.top()),
        };
        Some(if toward >= r.center().x { pos2(r.right(), y) } else { pos2(r.left(), y) })
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, bg) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let visuals = ui.visuals().clone();
        painter.rect_filled(rect, 0.0, if visuals.dark_mode { Color32::from_rgb(0x22, 0x22, 0x24) } else { Color32::from_rgb(0xF4, 0xF6, 0xF9) });
        // Dot grid.
        let step = 24.0;
        let off = vec2(self.pan.x.rem_euclid(step), self.pan.y.rem_euclid(step));
        let dot = if visuals.dark_mode { Color32::from_gray(60) } else { Color32::from_gray(205) };
        let mut y = rect.top() + off.y;
        while y < rect.bottom() {
            let mut x = rect.left() + off.x;
            while x < rect.right() {
                painter.circle_filled(pos2(x, y), 1.0, dot);
                x += step;
            }
            y += step;
        }
        let origin = rect.min;
        if self.tables.is_empty() {
            painter.text(rect.center(), Align2::CENTER_CENTER, "Trascina qui una tabella dall'elenco a sinistra (o doppio clic sul nome)", FontId::proportional(14.0), visuals.weak_text_color());
        }
        if bg.dragged() && self.drag_join.is_none() {
            self.pan += bg.drag_delta();
        }
        // The table being dragged from the list: a ghost under the pointer, and where it will land.
        if let (Some(name), Some(p)) = (egui::DragAndDrop::payload::<String>(ui.ctx()), ui.input(|i| i.pointer.hover_pos())) {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            if rect.contains(p) {
                painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0, ACCENT), StrokeKind::Inside);
                let fg = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("qb-ghost")));
                let card = Rect::from_min_size(p, vec2(TABLE_W, HEAD_H + 4.0 * ROW_H + 4.0));
                fg.rect_filled(card.translate(vec2(3.0, 4.0)), 6.0, Color32::from_black_alpha(25));
                fg.rect_filled(card, 6.0, ACCENT.gamma_multiply(0.12));
                fg.rect_stroke(card, 6.0, Stroke::new(1.5, ACCENT.gamma_multiply(0.8)), StrokeKind::Inside);
                let head = Rect::from_min_size(card.min, vec2(TABLE_W, HEAD_H));
                fg.rect_filled(head, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, ACCENT.gamma_multiply(0.75));
                fg.with_clip_rect(head.shrink(2.0)).text(head.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, name.as_str(), FontId::proportional(13.0), Color32::WHITE);
                for k in 0..4 {
                    let y = head.bottom() + 6.0 + k as f32 * ROW_H;
                    let w = [0.45, 0.6, 0.35, 0.5][k];
                    fg.rect_filled(Rect::from_min_size(pos2(card.left() + 10.0, y + 5.0), vec2((TABLE_W - 20.0) * w, 8.0)), 3.0, ACCENT.gamma_multiply(0.25));
                }
            } else {
                ghost_chip(ui.ctx(), p, &name, None);
            }
        }
        if let Some(name) = bg.dnd_release_payload::<String>() {
            let at = ui.input(|i| i.pointer.interact_pos()).map(|p| (p - origin - self.pan).to_pos2());
            self.add_table(&name, at);
        }

        // Join lines under the tables.
        let mut join_menu: Option<usize> = None;
        let mut remove_join = None;
        for (k, j) in self.joins.iter().enumerate() {
            let (Some(ta), Some(tb)) = (self.tables.get(j.a.0), self.tables.get(j.b.0)) else { continue };
            let (ra, rb) = (self.table_rect(ta, origin), self.table_rect(tb, origin));
            let (Some(pa), Some(pb)) = (self.anchor(j.a.0, &j.a.1, origin, rb.center().x), self.anchor(j.b.0, &j.b.1, origin, ra.center().x)) else { continue };
            let dx = ((pb.x - pa.x).abs() / 2.0).max(40.0);
            let ca = pos2(pa.x + if pa.x >= ra.center().x { dx } else { -dx }, pa.y);
            let cb = pos2(pb.x + if pb.x >= rb.center().x { dx } else { -dx }, pb.y);
            let color = Color32::from_rgb(0x1F, 0x6F, 0xD0);
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke([pa, ca, cb, pb], false, Color32::TRANSPARENT, Stroke::new(1.8, color)));
            painter.circle_filled(pa, 3.0, color);
            painter.circle_filled(pb, 3.0, color);
            let mid = pos2((pa.x + ca.x + cb.x + pb.x) / 4.0, (pa.y + ca.y + cb.y + pb.y) / 4.0);
            let badge = Rect::from_center_size(mid, vec2(24.0, 18.0));
            let r = ui.interact(badge, ui.scope_id().with(("join", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(i18n::t(j.kind.label()));
            painter.rect_filled(badge, 4.0, visuals.extreme_bg_color);
            painter.rect_stroke(badge, 4.0, Stroke::new(1.0, color), StrokeKind::Inside);
            painter.text(mid, Align2::CENTER_CENTER, j.kind.short(), FontId::proportional(13.0), color);
            egui::Popup::menu(&r).show(|ui| {
                for kind in JoinKind::ALL {
                    if ui.selectable_label(j.kind == kind, i18n::t(kind.label())).clicked() {
                        join_menu = Some(k);
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("qb-join-kind"), kind as u8));
                    }
                }
                ui.separator();
                if icons::menu_item(ui, G::Delete, "Elimina collegamento").clicked() {
                    remove_join = Some(k);
                }
            });
        }
        if let Some(k) = join_menu {
            let v = ui.ctx().data(|d| d.get_temp::<u8>(egui::Id::unique("qb-join-kind")));
            if let (Some(j), Some(v)) = (self.joins.get_mut(k), v) {
                j.kind = JoinKind::ALL.get(v as usize).copied().unwrap_or(JoinKind::Inner);
            }
        }
        if let Some(k) = remove_join {
            self.joins.remove(k);
        }

        // Tables.
        let mut close = None;
        let mut toggle: Option<(usize, String)> = None;
        let mut drop_target: Option<(usize, String)> = None;
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let released = ui.input(|i| i.pointer.any_released());
        let mut join_over: Option<(usize, String)> = None;
        for i in 0..self.tables.len() {
            let Some(t) = self.tables.get(i) else { continue };
            let r = self.table_rect(t, origin);
            if !rect.intersects(r) {
                continue;
            }
            // Shadow and body.
            painter.rect_filled(r.translate(vec2(2.0, 3.0)), 6.0, Color32::from_black_alpha(30));
            painter.rect_filled(r, 6.0, visuals.extreme_bg_color);
            painter.rect_stroke(r, 6.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);
            let head = Rect::from_min_size(r.min, vec2(TABLE_W, HEAD_H));
            painter.rect_filled(head, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, Color32::from_rgb(0x1F, 0x6F, 0xD0));
            let title = if t.alias == t.name { t.name.clone() } else { format!("{} ({})", t.name, t.alias) };
            painter.with_clip_rect(head.shrink(2.0)).text(head.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(13.0), Color32::WHITE);
            let close_r = Rect::from_center_size(head.right_center() - vec2(12.0, 0.0), vec2(16.0, 16.0));
            let close_resp = ui.interact(close_r, ui.scope_id().with(("qbclose", i)), Sense::click());
            painter.text(close_r.center(), Align2::CENTER_CENTER, "×", FontId::proportional(15.0), Color32::WHITE);
            if close_resp.clicked() {
                close = Some(i);
            }
            let head_resp = ui.interact(head.with_max_x(close_r.left() - 2.0), ui.scope_id().with(("qbhead", i)), Sense::drag()).on_hover_cursor(CursorIcon::Grab);
            let moved = head_resp.drag_delta();
            // Body rows.
            let body = Rect::from_min_max(pos2(r.left(), head.bottom()), r.max);
            let body_resp = ui.interact(body, ui.scope_id().with(("qbbody", i)), Sense::hover());
            let scroll_delta = if body_resp.hovered() { ui.input(|inp| inp.smooth_scroll_delta.y) } else { 0.0 };
            let clip = painter.with_clip_rect(body.shrink(1.0).intersect(rect));
            let t = &self.tables[i];
            if t.loading {
                clip.text(body.center(), Align2::CENTER_CENTER, "caricamento…", FontId::proportional(12.0), visuals.weak_text_color());
            } else if let Some(e) = &t.error {
                clip.text(body.left_top() + vec2(6.0, 6.0), Align2::LEFT_TOP, e, FontId::proportional(11.0), Color32::RED);
            }
            let names: Vec<(String, String, bool)> = std::iter::once(("*".to_string(), String::new(), false)).chain(t.columns.iter().map(|c| (c.name.clone(), c.data_type.clone(), c.primary_key))).collect();
            for (row, (name, ty, pk)) in names.iter().enumerate() {
                let Some(rr) = self.row_rect(t, origin, row) else { continue };
                let checked = self.fields.iter().any(|f| f.table == i && &f.col == name);
                let resp = ui.interact(rr.intersect(body), ui.scope_id().with(("qbrow", i, row)), Sense::click_and_drag());
                let dragging_from = self.drag_join.as_ref().map(|(ti, c)| (*ti, c == name));
                if dragging_from == Some((i, true)) {
                    clip.rect_filled(rr, 0.0, ACCENT.gamma_multiply(0.18));
                } else if let Some((src, _)) = dragging_from
                    && src != i
                    && row > 0
                    && pointer.is_some_and(|p| rr.contains(p))
                {
                    clip.rect_filled(rr, 0.0, OK.gamma_multiply(0.22));
                    clip.rect_stroke(rr, 0.0, Stroke::new(1.5, OK), StrokeKind::Inside);
                    join_over = Some((i, name.clone()));
                } else if resp.hovered() {
                    clip.rect_filled(rr, 0.0, visuals.widgets.hovered.weak_bg_fill);
                }
                let cb = Rect::from_center_size(rr.left_center() + vec2(14.0, 0.0), vec2(12.0, 12.0));
                clip.rect_stroke(cb, 2.0, Stroke::new(1.0, visuals.text_color()), StrokeKind::Inside);
                if checked {
                    icons::paint(&clip, cb.expand(1.0), G::Check, Color32::from_rgb(0x1F, 0x6F, 0xD0));
                }
                let mut x = rr.left() + 26.0;
                if *pk {
                    icons::paint(&clip, Rect::from_center_size(pos2(x + 6.0, rr.center().y), vec2(13.0, 13.0)), Db::Key, visuals.text_color());
                    x += 14.0;
                }
                clip.text(pos2(x, rr.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.5), visuals.text_color());
                clip.text(rr.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, ty, FontId::proportional(10.0), visuals.weak_text_color());
                if resp.clicked() {
                    toggle = Some((i, name.clone()));
                }
                if row > 0 && resp.drag_started() {
                    self.drag_join = Some((i, name.clone()));
                }
                if row > 0 && released && pointer.is_some_and(|p| rr.contains(p)) {
                    drop_target = Some((i, name.clone()));
                }
            }
            if let Some(tm) = self.tables.get_mut(i) {
                if moved != Vec2::ZERO {
                    tm.pos += moved;
                }
                if scroll_delta != 0.0 {
                    let max = ((names.len() as f32) * ROW_H - (r.height() - HEAD_H - 4.0)).max(0.0);
                    tm.scroll = (tm.scroll - scroll_delta).clamp(0.0, max);
                }
            }
        }
        // Join being dragged.
        if let (Some((ti, col)), Some(p)) = (&self.drag_join, pointer)
            && let Some(start) = self.anchor(*ti, col, origin, p.x)
        {
            let color = if join_over.is_some() { OK } else { ACCENT };
            let dx = ((p.x - start.x).abs() / 2.0).max(30.0);
            let c1 = pos2(start.x + if p.x >= start.x { dx } else { -dx }, start.y);
            let c2 = pos2(p.x - if p.x >= start.x { dx } else { -dx }, p.y);
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke([start, c1, c2, p], false, Color32::TRANSPARENT, Stroke::new(2.0, color)));
            painter.circle_filled(start, 3.5, color);
            painter.circle_filled(p, 4.0, color);
            let from = self.tables.get(*ti).map(|t| format!("{}.{col}", t.alias)).unwrap_or_else(|| col.clone());
            let to = join_over.as_ref().and_then(|(tj, c)| self.tables.get(*tj).map(|t| format!("{}.{c}", t.alias)));
            ghost_chip(ui.ctx(), p, &from, Some(to.as_deref().unwrap_or("?")));
            ui.ctx().set_cursor_icon(if join_over.is_some() { CursorIcon::Copy } else { CursorIcon::Grabbing });
        }
        if released && let Some((ti, col)) = self.drag_join.take()
            && let Some((tj, col2)) = drop_target
            && tj != ti
            && !self.joins.iter().any(|j| (j.a == (ti, col.clone()) && j.b == (tj, col2.clone())) || (j.a == (tj, col2.clone()) && j.b == (ti, col.clone())))
        {
            self.joins.push(BJoin { a: (ti, col), b: (tj, col2), kind: JoinKind::Inner });
        }
        if let Some((i, c)) = toggle {
            self.toggle_field(i, &c);
        }
        if let Some(i) = close {
            self.remove_table(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::filter::{Item, Op, Sort};

    fn col(name: &str, ty: &str, pk: bool) -> ColumnInfo {
        ColumnInfo { name: name.into(), data_type: ty.into(), primary_key: pk, nullable: true, ..Default::default() }
    }

    fn builder(d: Dialect) -> QueryBuilder {
        let mut b = QueryBuilder::new(d, None);
        b.add_table("customers", None);
        b.add_table("orders", None);
        b.on_table_info("customers", Ok(TableInfo { name: "customers".into(), columns: vec![col("id", "int", true), col("name", "text", false), col("city", "text", false)], ..Default::default() }));
        b.on_table_info(
            "orders",
            Ok(TableInfo {
                name: "orders".into(),
                columns: vec![col("id", "int", true), col("customer_id", "int", false), col("total", "numeric", false)],
                foreign_keys: vec![ForeignKeyInfo { name: "fk".into(), columns: vec!["customer_id".into()], ref_table: "customers".into(), ref_columns: vec!["id".into()], ..Default::default() }],
                ..Default::default()
            }),
        );
        b
    }

    #[test]
    fn foreign_keys_join_automatically() {
        let b = builder(Dialect::Postgres);
        assert_eq!(b.joins.len(), 1);
        assert_eq!(b.sql(), "SELECT *\nFROM \"customers\"\nINNER JOIN \"orders\" ON \"customers\".\"id\" = \"orders\".\"customer_id\"");
    }

    #[test]
    fn fields_aggregates_conditions_order_limit() {
        let mut b = builder(Dialect::MySql);
        b.toggle_field(0, "city");
        b.toggle_field(1, "total");
        b.fields[1].agg = Agg::Sum;
        b.fields[1].alias = "fatturato".into();
        b.filter.push(Item::cond("customers.name", Op::Begins, "A"));
        b.filter.sorts.push(Sort { col: "customers.city".into(), desc: true });
        b.joins[0].kind = JoinKind::Left;
        b.limit = "10".into();
        b.distinct = true;
        assert_eq!(
            b.sql(),
            "SELECT DISTINCT `customers`.`city`,\n       SUM(`orders`.`total`) AS `fatturato`\nFROM `customers`\nLEFT JOIN `orders` ON `customers`.`id` = `orders`.`customer_id`\nWHERE `customers`.`name` LIKE 'A%' ESCAPE '\\\\'\nGROUP BY `customers`.`city`\nORDER BY `customers`.`city` DESC\nLIMIT 10"
        );
    }

    #[test]
    fn sql_server_top_and_self_join_alias() {
        let mut b = builder(Dialect::SqlServer);
        b.add_table("customers", None);
        assert_eq!(b.tables[2].alias, "customers2");
        b.on_table_info("customers", Ok(TableInfo { name: "customers".into(), columns: vec![col("id", "int", true)], ..Default::default() }));
        b.limit = "5".into();
        let sql = b.sql();
        assert!(sql.starts_with("SELECT TOP 5 *"), "{sql}");
        // The second copy is joined through the orders → customers foreign key, under its alias.
        assert!(sql.contains("INNER JOIN [customers] [customers2] ON [orders].[customer_id] = [customers2].[id]"), "{sql}");
        assert!(!sql.contains("LIMIT"));
    }

    #[test]
    fn removing_a_table_drops_its_joins_and_fields() {
        let mut b = builder(Dialect::Sqlite);
        b.toggle_field(1, "total");
        b.toggle_field(0, "name");
        b.remove_table(0);
        assert!(b.joins.is_empty());
        assert_eq!(b.fields, vec![BField { table: 0, col: "total".into(), alias: String::new(), agg: Agg::None }]);
        assert_eq!(b.sql(), "SELECT \"orders\".\"total\"\nFROM \"orders\"");
    }

    #[test]
    fn right_join_reads_from_the_placed_side() {
        let mut b = builder(Dialect::Postgres);
        // Join drawn from orders (b) to customers (a) as LEFT: orders keeps all rows.
        b.joins[0] = BJoin { a: (1, "customer_id".into()), b: (0, "id".into()), kind: JoinKind::Left };
        assert!(b.sql().contains("RIGHT JOIN \"orders\" ON \"orders\".\"customer_id\" = \"customers\".\"id\""), "{}", b.sql());
    }
}
