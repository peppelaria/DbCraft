//! "Filtro e ordinamento", modelled on Navicat's: each condition reads as a sentence of clickable
//! tokens (field · operator · value), conditions are linked by their own and/or connector, and
//! bracketed groups nest. The sort list works the same way. A raw-SQL mode keeps a free WHERE.

use db_core::Dialect;
use dbgrid::{ColumnKind, GridColumn};
use egui::{Color32, RichText, Sense};
use crate::i18n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    NotContains,
    Begins,
    NotBegins,
    Ends,
    NotEnds,
    IsNull,
    IsNotNull,
    IsEmpty,
    IsNotEmpty,
    Between,
    NotBetween,
    In,
    NotIn,
    /// A free SQL condition (the value), ignoring the field.
    Custom,
}

impl Op {
    pub const ALL: [Op; 21] = [
        Op::Eq,
        Op::Ne,
        Op::Lt,
        Op::Le,
        Op::Gt,
        Op::Ge,
        Op::Contains,
        Op::NotContains,
        Op::Begins,
        Op::NotBegins,
        Op::Ends,
        Op::NotEnds,
        Op::IsNull,
        Op::IsNotNull,
        Op::IsEmpty,
        Op::IsNotEmpty,
        Op::Between,
        Op::NotBetween,
        Op::In,
        Op::NotIn,
        Op::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Op::Eq => "=",
            Op::Ne => "!=",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Contains => "contiene",
            Op::NotContains => "non contiene",
            Op::Begins => "inizia con",
            Op::NotBegins => "non inizia con",
            Op::Ends => "finisce con",
            Op::NotEnds => "non finisce con",
            Op::IsNull => "è null",
            Op::IsNotNull => "non è null",
            Op::IsEmpty => "è vuoto",
            Op::IsNotEmpty => "non è vuoto",
            Op::Between => "è compreso tra",
            Op::NotBetween => "non è compreso tra",
            Op::In => "è nell'elenco",
            Op::NotIn => "non è nell'elenco",
            Op::Custom => "[Personalizzato]",
        }
    }

    fn values(self) -> usize {
        match self {
            Op::IsNull | Op::IsNotNull | Op::IsEmpty | Op::IsNotEmpty => 0,
            Op::Between | Op::NotBetween => 2,
            _ => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Join {
    #[default]
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cond {
    pub col: String,
    pub op: Op,
    pub value: String,
    pub value2: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Cond(Cond),
    Group(Vec<Item>),
}

/// A condition or group, with the connector that links it to the next item.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub enabled: bool,
    pub join: Join,
    pub node: Node,
}

impl Item {
    pub fn cond(col: impl Into<String>, op: Op, value: impl Into<String>) -> Item {
        Item { enabled: true, join: Join::And, node: Node::Cond(Cond { col: col.into(), op, value: value.into(), value2: String::new() }) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sort {
    pub col: String,
    pub desc: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilterSort {
    pub items: Vec<Item>,
    pub sorts: Vec<Sort>,
    /// Free-text WHERE instead of the builder.
    pub raw_mode: bool,
    pub raw: String,
    /// Column names are `alias.column` (query builder): quote each part.
    pub qualified: bool,
    /// Inside another editor: no Apply/Reset buttons.
    pub embedded: bool,
    selected: Option<Vec<usize>>,
    editing: Option<(Vec<usize>, usize)>,
}

fn literal(d: Dialect, kind: ColumnKind, v: &str) -> String {
    let t = v.trim();
    if matches!(kind, ColumnKind::Number) && !t.is_empty() && t.parse::<f64>().is_ok() {
        t.to_string()
    } else {
        d.string_literal(v)
    }
}

/// `LIKE` pattern literal for a user value: `%` and `_` in the value are matched literally.
fn like(d: Dialect, v: &str, pre: &str, post: &str) -> String {
    let escaped = v.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("{} ESCAPE {}", d.string_literal(&format!("{pre}{escaped}{post}")), d.string_literal("\\"))
}

impl Cond {
    pub fn sql(&self, d: Dialect, kind: ColumnKind, qualified: bool) -> Option<String> {
        if self.op == Op::Custom {
            return Some(self.value.trim().to_string()).filter(|s| !s.is_empty()).map(|s| format!("({s})"));
        }
        if self.col.is_empty() {
            return None;
        }
        let c = if qualified { quote_path(d, &self.col) } else { d.quote(&self.col) };
        let lit = |v: &str| literal(d, kind, v);
        let list = |v: &str| v.split(',').map(|x| lit(x.trim())).collect::<Vec<_>>().join(", ");
        // LIKE on a non-text column compares its text form.
        let text_col = match (kind, d) {
            (ColumnKind::Text, _) => c.clone(),
            (_, Dialect::Postgres) => format!("CAST({c} AS TEXT)"),
            (_, Dialect::SqlServer) => format!("CAST({c} AS NVARCHAR(MAX))"),
            _ => c.clone(),
        };
        Some(match self.op {
            Op::Eq => format!("{c} = {}", lit(&self.value)),
            Op::Ne => format!("{c} <> {}", lit(&self.value)),
            Op::Lt => format!("{c} < {}", lit(&self.value)),
            Op::Le => format!("{c} <= {}", lit(&self.value)),
            Op::Gt => format!("{c} > {}", lit(&self.value)),
            Op::Ge => format!("{c} >= {}", lit(&self.value)),
            Op::Contains => format!("{text_col} LIKE {}", like(d, &self.value, "%", "%")),
            Op::NotContains => format!("{text_col} NOT LIKE {}", like(d, &self.value, "%", "%")),
            Op::Begins => format!("{text_col} LIKE {}", like(d, &self.value, "", "%")),
            Op::NotBegins => format!("{text_col} NOT LIKE {}", like(d, &self.value, "", "%")),
            Op::Ends => format!("{text_col} LIKE {}", like(d, &self.value, "%", "")),
            Op::NotEnds => format!("{text_col} NOT LIKE {}", like(d, &self.value, "%", "")),
            Op::IsNull => format!("{c} IS NULL"),
            Op::IsNotNull => format!("{c} IS NOT NULL"),
            Op::IsEmpty => format!("({c} IS NULL OR {text_col} = '')"),
            Op::IsNotEmpty => format!("({c} IS NOT NULL AND {text_col} <> '')"),
            Op::Between => format!("{c} BETWEEN {} AND {}", lit(&self.value), lit(&self.value2)),
            Op::NotBetween => format!("{c} NOT BETWEEN {} AND {}", lit(&self.value), lit(&self.value2)),
            Op::In => format!("{c} IN ({})", list(&self.value)),
            Op::NotIn => format!("{c} NOT IN ({})", list(&self.value)),
            Op::Custom => return None,
        })
    }
}

/// `a.b` → quoted `a`.`b`.
pub fn quote_path(d: Dialect, path: &str) -> String {
    match path.split_once('.') {
        Some((a, b)) => format!("{}.{}", d.quote(a), d.quote(b)),
        None => d.quote(path),
    }
}

fn items_sql(items: &[Item], d: Dialect, kind: &dyn Fn(&str) -> ColumnKind, qualified: bool) -> Option<String> {
    let parts: Vec<(String, Join)> = items
        .iter()
        .filter(|i| i.enabled)
        .filter_map(|i| {
            let s = match &i.node {
                Node::Cond(c) => c.sql(d, kind(&c.col), qualified),
                Node::Group(g) => items_sql(g, d, kind, qualified).map(|s| format!("({s})")),
            };
            s.map(|s| (s, i.join))
        })
        .collect();
    let mut out = String::new();
    for (k, (s, _)) in parts.iter().enumerate() {
        if k > 0 {
            out.push_str(if parts[k - 1].1 == Join::Or { " OR " } else { " AND " });
        }
        out.push_str(s);
    }
    (!out.is_empty()).then_some(out)
}

/// Item at `path` (indexes from the root, through groups).
fn item_mut<'a>(items: &'a mut [Item], path: &[usize]) -> Option<&'a mut Item> {
    let (first, rest) = path.split_first()?;
    let item = items.get_mut(*first)?;
    if rest.is_empty() {
        return Some(item);
    }
    match &mut item.node {
        Node::Group(g) => item_mut(g, rest),
        Node::Cond(_) => None,
    }
}

/// The list holding the item at `path`, and its index there.
fn parent_mut<'a>(items: &'a mut Vec<Item>, path: &[usize]) -> Option<(&'a mut Vec<Item>, usize)> {
    let (last, parent) = path.split_last()?;
    if parent.is_empty() {
        return Some((items, *last));
    }
    match &mut item_mut(items, parent)?.node {
        Node::Group(g) => Some((g, *last)),
        Node::Cond(_) => None,
    }
}

enum Act {
    Select(Vec<usize>),
    AddCond(Vec<usize>),
    AddGroup(Vec<usize>),
    /// Add a condition as the first item inside the group at the path.
    AddInside(Vec<usize>),
    Delete(Vec<usize>),
    Edit(Vec<usize>, usize),
}

const LINK: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xD0);
const FIELD: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xD0);
const VALUE: Color32 = Color32::from_rgb(0x3B, 0x4E, 0xC8);

fn token(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(egui::Label::new(RichText::new(i18n::tr(text)).color(color)).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl FilterSort {
    pub fn where_sql(&self, d: Dialect, columns: &[GridColumn]) -> Option<String> {
        if self.raw_mode {
            return Some(self.raw.trim().to_string()).filter(|s| !s.is_empty());
        }
        let kind = |name: &str| columns.iter().find(|c| c.name == name).map(|c| c.kind).unwrap_or_default();
        items_sql(&self.items, d, &kind, self.qualified)
    }

    pub fn order(&self) -> Vec<(String, bool)> {
        self.sorts.iter().filter(|s| !s.col.is_empty()).map(|s| (s.col.clone(), s.desc)).collect()
    }

    /// Embedded in the query builder: qualified `alias.column` names, no Apply button.
    pub fn for_builder() -> Self {
        FilterSort { qualified: true, embedded: true, ..Default::default() }
    }

    pub fn select(&mut self, path: Vec<usize>) {
        self.selected = Some(path);
    }

    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    fn apply_act(&mut self, act: Act, first_col: &str) {
        let new_cond = || Item::cond(first_col, Op::Eq, "");
        let group = matches!(act, Act::AddGroup(_));
        match act {
            Act::Select(p) => self.selected = Some(p),
            Act::Edit(p, k) => self.editing = Some((p, k)),
            Act::AddCond(p) | Act::AddGroup(p) if p.is_empty() => {
                self.items.push(if group { Item { enabled: true, join: Join::And, node: Node::Group(vec![new_cond()]) } } else { new_cond() });
                self.selected = Some(vec![self.items.len() - 1]);
            }
            Act::AddCond(p) | Act::AddGroup(p) => {
                if let Some((list, i)) = parent_mut(&mut self.items, &p) {
                    let item = if group { Item { enabled: true, join: Join::And, node: Node::Group(vec![new_cond()]) } } else { new_cond() };
                    list.insert(i + 1, item);
                    let mut sel = p.clone();
                    if let Some(last) = sel.last_mut() {
                        *last = i + 1;
                    }
                    self.selected = Some(sel);
                }
            }
            Act::AddInside(p) => {
                if let Some(Item { node: Node::Group(g), .. }) = item_mut(&mut self.items, &p) {
                    g.insert(0, new_cond());
                    let mut sel = p.clone();
                    sel.push(0);
                    self.selected = Some(sel);
                }
            }
            Act::Delete(p) => {
                if let Some((list, i)) = parent_mut(&mut self.items, &p)
                    && i < list.len()
                {
                    list.remove(i);
                }
                self.selected = None;
                self.editing = None;
            }
        }
    }

    /// Draws the panel. Returns true when the user asks to apply it.
    pub fn ui(&mut self, ui: &mut egui::Ui, columns: &[GridColumn], d: Dialect) -> bool {
        let mut apply = false;
        let first = columns.first().map(|c| c.name.clone()).unwrap_or_default();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.strong(i18n::t("Filtro"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut self.raw_mode, i18n::t("SQL")).on_hover_text(i18n::t("Scrivi il WHERE a mano"));
                });
            });
            if self.raw_mode {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(i18n::t("WHERE")).monospace());
                    let r = ui.add(egui::TextEdit::singleline(&mut self.raw).desired_width(ui.available_width() - 10.0).font(egui::TextStyle::Monospace).hint_text(i18n::t("es. id > 10 AND nome LIKE 'A%'")));
                    apply |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                });
            } else {
                let mut acts = Vec::new();
                let mut items = std::mem::take(&mut self.items);
                egui::ScrollArea::vertical().max_height(220.0).id_salt("filter-items").show(ui, |ui| {
                    if items.is_empty() && token(ui, "<clicca qui per aggiungere una condizione>", Color32::GRAY).clicked() {
                        acts.push(Act::AddCond(Vec::new()));
                    }
                    let n = items.len();
                    for (i, item) in items.iter_mut().enumerate() {
                        self.item_ui(ui, item, vec![i], i + 1 == n, 0, columns, &mut acts, &mut apply);
                    }
                });
                self.items = items;
                for a in acts {
                    self.apply_act(a, &first);
                }
            }
            ui.add_space(4.0);
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.strong(i18n::t("Ordina per"));
                ui.add_space(8.0);
                let mut remove = None;
                let n = self.sorts.len();
                for (i, s) in self.sorts.iter_mut().enumerate() {
                    let r = token(ui, &s.col, FIELD);
                    egui::Popup::menu(&r).show(|ui| {
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for c in columns {
                                if ui.selectable_label(s.col == c.name, &c.name).clicked() {
                                    s.col = c.name.clone();
                                }
                            }
                        });
                    });
                    if token(ui, if s.desc { "DESC" } else { "ASC" }, Color32::DARK_GRAY).on_hover_text(i18n::t("Clic per invertire")).clicked() {
                        s.desc = !s.desc;
                    }
                    if ui.small_button("×").on_hover_text(i18n::t("Rimuovi")).clicked() {
                        remove = Some(i);
                    }
                    if i + 1 < n {
                        ui.label(",");
                    }
                }
                if let Some(i) = remove {
                    self.sorts.remove(i);
                }
                if self.sorts.is_empty() {
                    if token(ui, "<clicca qui per aggiungere un ordinamento>", Color32::GRAY).clicked() {
                        self.sorts.push(Sort { col: first.clone(), desc: false });
                    }
                } else if ui.small_button("+").on_hover_text(i18n::t("Aggiungi ordinamento")).clicked() {
                    self.sorts.push(Sort { col: first.clone(), desc: false });
                }
            });
            ui.add_space(4.0);
            if self.embedded {
                return;
            }
            ui.horizontal(|ui| {
                let btn = egui::Button::new(RichText::new(i18n::t("Applica filtro e ordinamento")).color(Color32::WHITE).strong()).fill(LINK);
                if ui.add(btn).clicked() {
                    apply = true;
                }
                if ui.button(i18n::t("Azzera")).clicked() {
                    *self = FilterSort { raw_mode: self.raw_mode, qualified: self.qualified, ..Default::default() };
                    apply = true;
                }
                if let Some(w) = self.where_sql(d, columns) {
                    ui.label(RichText::new(i18n::tf(format!("WHERE {w}"))).monospace().small().weak());
                }
            });
        });
        apply
    }

    #[allow(clippy::too_many_arguments)]
    fn item_ui(&self, ui: &mut egui::Ui, item: &mut Item, path: Vec<usize>, last: bool, depth: usize, columns: &[GridColumn], acts: &mut Vec<Act>, apply: &mut bool) {
        let selected = self.selected.as_ref() == Some(&path);
        let fill = if selected { ui.visuals().selection.bg_fill.gamma_multiply(0.35) } else { Color32::TRANSPARENT };
        // The row's click (to select it) goes below its tokens: a click sense added to the Frame's
        // response afterwards would be on top of them and take their clicks.
        let row = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
            egui::Frame::new().fill(fill).inner_margin(egui::Margin::symmetric(4, 2)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 22.0);
                    crate::theme::checkbox(ui, &mut item.enabled, "");
                    match &mut item.node {
                        Node::Group(_) => {
                            ui.label(RichText::new("(").strong());
                        }
                        Node::Cond(c) => {
                            ui.add_enabled_ui(item.enabled, |ui| self.cond_ui(ui, c, &path, columns, acts, apply));
                        }
                    }
                    if matches!(item.node, Node::Cond(_)) && !last {
                        join_token(ui, &mut item.join);
                    }
                    if selected {
                        ui.add_space(12.0);
                        if matches!(item.node, Node::Group(_)) && ui.small_button("+").on_hover_text(i18n::t("Aggiungi una condizione nel gruppo")).clicked() {
                            acts.push(Act::AddInside(path.clone()));
                        }
                        if ui.small_button("|+").on_hover_text(i18n::t("Aggiungi una condizione dopo questa")).clicked() {
                            acts.push(Act::AddCond(path.clone()));
                        }
                        if ui.small_button("()+").on_hover_text(i18n::t("Aggiungi un gruppo tra parentesi")).clicked() {
                            acts.push(Act::AddGroup(path.clone()));
                        }
                        if crate::icons::small(ui, crate::icons::G::Delete, "Elimina", true).clicked() {
                            acts.push(Act::Delete(path.clone()));
                        }
                    }
                });
            });
        });
        if row.response.clicked() {
            acts.push(Act::Select(path.clone()));
        }
        if let Node::Group(children) = &mut item.node {
            let n = children.len();
            for (i, child) in children.iter_mut().enumerate() {
                let mut p = path.clone();
                p.push(i);
                self.item_ui(ui, child, p, i + 1 == n, depth + 1, columns, acts, apply);
            }
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 22.0 + 30.0);
                ui.label(RichText::new(")").strong());
                if !last {
                    join_token(ui, &mut item.join);
                }
            });
        }
    }

    fn cond_ui(&self, ui: &mut egui::Ui, c: &mut Cond, path: &[usize], columns: &[GridColumn], acts: &mut Vec<Act>, apply: &mut bool) {
        if c.op != Op::Custom {
            let r = token(ui, if c.col.is_empty() { "<campo>" } else { &c.col }, FIELD);
            if r.clicked() {
                acts.push(Act::Select(path.to_vec()));
            }
            egui::Popup::menu(&r).show(|ui| {
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for col in columns {
                        if ui.selectable_label(c.col == col.name, &col.name).clicked() {
                            c.col = col.name.clone();
                        }
                    }
                });
            });
        }
        let r = token(ui, i18n::t(c.op.label()), Color32::DARK_GRAY);
        egui::Popup::menu(&r).show(|ui| {
            for op in Op::ALL {
                if ui.selectable_label(c.op == op, i18n::t(op.label())).clicked() {
                    c.op = op;
                }
            }
        });
        let n = c.op.values();
        for k in 0..n {
            if k == 1 {
                ui.label(i18n::t("e"));
            }
            let editing = self.editing.as_ref().is_some_and(|(p, kk)| p == path && *kk == k);
            let slot = if k == 0 { &mut c.value } else { &mut c.value2 };
            if editing {
                let width = if c.op == Op::Custom { 360.0 } else { 160.0 };
                let r = ui.add(egui::TextEdit::singleline(slot).desired_width(width).hint_text(match c.op {
                    Op::In | Op::NotIn => i18n::t("a, b, c"),
                    Op::Custom => i18n::t("es. LENGTH(nome) > 3"),
                    _ => i18n::t("valore"),
                }));
                if !r.has_focus() && !r.lost_focus() {
                    r.request_focus();
                }
                if r.lost_focus() {
                    *apply |= ui.input(|i| i.key_pressed(egui::Key::Enter));
                    acts.push(Act::Edit(Vec::new(), usize::MAX));
                }
            } else {
                let shown = if slot.is_empty() { "<?>".to_string() } else if c.op == Op::Custom { slot.clone() } else { format!("'{slot}'") };
                if token(ui, &shown, VALUE).on_hover_text(i18n::t("Clic per modificare")).clicked() {
                    acts.push(Act::Edit(path.to_vec(), k));
                    acts.push(Act::Select(path.to_vec()));
                }
            }
        }
    }
}

fn join_token(ui: &mut egui::Ui, join: &mut Join) {
    let text = if *join == Join::And { "and" } else { "or" };
    if token(ui, text, Color32::from_rgb(0xC2, 0x5E, 0x00)).on_hover_text(i18n::t("Clic per alternare and/or")).clicked() {
        *join = if *join == Join::And { Join::Or } else { Join::And };
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn cols() -> Vec<GridColumn> {
        vec![
            GridColumn { name: "id".into(), kind: ColumnKind::Number, ..Default::default() },
            GridColumn { name: "name".into(), kind: ColumnKind::Text, ..Default::default() },
        ]
    }

    fn cond(col: &str, op: Op, v: &str) -> Item {
        let mut i = Item::cond(col, op, v);
        if let Node::Cond(c) = &mut i.node {
            c.value2 = "20".into();
        }
        i
    }

    fn fs(items: Vec<Item>) -> FilterSort {
        FilterSort { items, ..Default::default() }
    }

    #[test]
    fn builds_where() {
        let mut f = fs(vec![cond("id", Op::Gt, "10"), cond("name", Op::Contains, "50%_off")]);
        assert_eq!(f.where_sql(Dialect::Postgres, &cols()).unwrap(), "\"id\" > 10 AND \"name\" LIKE '%50\\%\\_off%' ESCAPE '\\'");
        f.items[1].enabled = false;
        assert_eq!(f.where_sql(Dialect::MySql, &cols()).unwrap(), "`id` > 10");
        let f = fs(vec![cond("id", Op::In, "1, 2,x"), cond("id", Op::NotBetween, "5")]);
        assert_eq!(f.where_sql(Dialect::Sqlite, &cols()).unwrap(), "\"id\" IN (1, 2, 'x') AND \"id\" NOT BETWEEN 5 AND 20");
        let f = fs(vec![cond("id", Op::Contains, "4")]);
        assert!(f.where_sql(Dialect::Postgres, &cols()).unwrap().starts_with("CAST(\"id\" AS TEXT) LIKE"));
        let f = fs(vec![cond("name", Op::IsEmpty, "")]);
        assert_eq!(f.where_sql(Dialect::SqlServer, &cols()).unwrap(), "([name] IS NULL OR [name] = '')");
        let f = FilterSort { raw_mode: true, raw: "  ".into(), ..Default::default() };
        assert_eq!(f.where_sql(Dialect::Sqlite, &cols()), None);
    }

    #[test]
    fn groups_and_connectors() {
        // id = 1 OR (name begins with 'a' AND name not ends with 'z')
        let mut a = cond("id", Op::Eq, "1");
        a.join = Join::Or;
        let group = Item { enabled: true, join: Join::And, node: Node::Group(vec![cond("name", Op::Begins, "a"), cond("name", Op::NotEnds, "z")]) };
        let f = fs(vec![a, group]);
        assert_eq!(f.where_sql(Dialect::Sqlite, &cols()).unwrap(), "\"id\" = 1 OR (\"name\" LIKE 'a%' ESCAPE '\\' AND \"name\" NOT LIKE '%z' ESCAPE '\\')");
        // Empty and disabled groups vanish.
        let f = fs(vec![Item { enabled: true, join: Join::And, node: Node::Group(vec![]) }, cond("id", Op::IsNull, "")]);
        assert_eq!(f.where_sql(Dialect::Sqlite, &cols()).unwrap(), "\"id\" IS NULL");
    }

    #[test]
    fn custom_condition() {
        let f = fs(vec![cond("", Op::Custom, "LENGTH(name) > 3"), cond("id", Op::Gt, "1")]);
        assert_eq!(f.where_sql(Dialect::Sqlite, &cols()).unwrap(), "(LENGTH(name) > 3) AND \"id\" > 1");
    }

    #[test]
    fn tree_editing() {
        let mut f = fs(vec![cond("id", Op::Eq, "1")]);
        f.apply_act(Act::AddGroup(vec![0]), "id");
        f.apply_act(Act::AddInside(vec![1]), "name");
        assert!(matches!(&f.items[1].node, Node::Group(g) if g.len() == 2));
        f.apply_act(Act::Delete(vec![1, 0]), "id");
        assert!(matches!(&f.items[1].node, Node::Group(g) if g.len() == 1));
        f.apply_act(Act::Delete(vec![0]), "id");
        assert_eq!(f.items.len(), 1);
    }

    /// Draws the panel headless and clicks the widget that `pick` chooses among the clickable ones
    /// (in drawing order, with what they say about themselves) at the `spot` of its rectangle;
    /// returns the labels on screen afterwards.
    fn click_on(f: &mut FilterSort, pick: impl Fn(&[Option<egui::WidgetInfo>]) -> Option<usize>, spot: fn(egui::Rect) -> egui::Pos2) -> Vec<String> {
        let ctx = egui::Context::default();
        // as in main.rs
        ctx.set_theme(egui::Theme::Light);
        crate::app::contrast(&ctx);
        crate::theme::install(&ctx);
        ctx.global_style_mut(|s| s.debug.show_interactive_widgets = true);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let widgets = |ctx: &egui::Context| -> Vec<(Option<egui::WidgetInfo>, egui::Rect)> {
            ctx.viewport(|vp| {
                let w = &vp.this_pass.widgets;
                w.layer_ids().flat_map(|l| w.get_layer(l).filter(|r| r.sense.senses_click()).map(|r| (w.info(r.id).cloned(), r.rect)).collect::<Vec<_>>()).collect()
            })
        };
        let pass = |ctx: &egui::Context, events: Vec<egui::Event>, f: &mut FilterSort| {
            let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                f.ui(ui, &cols(), Dialect::MySql);
            });
            out.textures_delta.clear();
        };
        pass(&ctx, vec![], f);
        pass(&ctx, vec![], f);
        let all = widgets(&ctx);
        let infos: Vec<_> = all.iter().map(|(i, _)| i.clone()).collect();
        let at = spot(all[pick(&infos).unwrap()].1);
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        pass(&ctx, vec![egui::Event::PointerMoved(at)], f);
        pass(&ctx, vec![button(true)], f);
        pass(&ctx, vec![button(false)], f);
        pass(&ctx, vec![], f);
        widgets(&ctx).into_iter().filter_map(|(i, _)| i?.label).collect()
    }

    fn click_token(f: &mut FilterSort, label: &str) -> Vec<String> {
        click_on(f, |w| w.iter().position(|i| i.as_ref().is_some_and(|i| i.label.as_deref() == Some(label))), |r| r.center())
    }

    fn checkbox(w: &[Option<egui::WidgetInfo>]) -> Option<usize> {
        w.iter().position(|i| i.as_ref().is_some_and(|i| i.role == egui::Role::CheckBox))
    }

    /// One condition `id = <?>`, selected (its buttons shown).
    fn one_selected() -> FilterSort {
        let mut f = fs(vec![Item::cond("id", Op::Eq, "")]);
        f.select(vec![0]);
        f
    }

    #[test]
    fn everything_in_a_condition_row_takes_its_own_clicks() {
        // the value: becomes a text field
        let mut f = one_selected();
        click_token(&mut f, "<?>");
        assert_eq!(f.editing, Some((vec![0], 0)));
        // the operator: opens the list of operators
        let mut f = one_selected();
        assert!(click_token(&mut f, "=").iter().any(|l| l == i18n::t(Op::Contains.label())));
        // the field: opens the list of columns
        let mut f = one_selected();
        assert!(click_token(&mut f, "id").iter().any(|l| l == "name"));
        // the check box: turns the condition off
        let mut f = one_selected();
        click_on(&mut f, checkbox, |r| r.center());
        assert!(!f.items[0].enabled);
        // |+ adds a condition, ()+ a group
        let mut f = one_selected();
        click_token(&mut f, "|+");
        assert_eq!(f.items.len(), 2);
        let mut f = one_selected();
        click_token(&mut f, "()+");
        assert!(matches!(f.items.get(1), Some(Item { node: Node::Group(_), .. })));
        // the bin (an icon, which doesn't describe itself: the one after ()+) removes it
        let mut f = one_selected();
        click_on(&mut f, |w| w.iter().position(|i| i.as_ref().is_some_and(|i| i.label.as_deref() == Some("()+"))).map(|k| k + 1), |r| r.center());
        assert!(f.items.is_empty());
        // a click on the empty part of the row (the clickable thing just before its check box,
        // near its right end) still selects it
        let mut f = fs(vec![Item::cond("id", Op::Eq, "")]);
        click_on(&mut f, |w| checkbox(w)?.checked_sub(1), |r| r.right_center() - egui::vec2(10.0, 0.0));
        assert_eq!(f.selected, Some(vec![0]));
    }

    #[test]
    fn mysql_escapes_backslash_in_like() {
        let f = fs(vec![cond("name", Op::Begins, "a")]);
        assert_eq!(f.where_sql(Dialect::MySql, &cols()).unwrap(), "`name` LIKE 'a%' ESCAPE '\\\\'");
    }
}
