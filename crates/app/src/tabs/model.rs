//! The "Modello" tab: the ER diagram of a database/schema, as Navicat's Model. A box per table
//! with its columns (key icon on the primary key, the foreign keys marked), a line per foreign key
//! from the referencing table to the referenced one. Boxes can be dragged, the sheet panned
//! (drag the background) and zoomed; "Esporta SQL" writes the CREATE TABLE of the whole model.

use std::collections::HashMap;

use db_core::{Dialect, ObjectKind, TableDesign, TableInfo};
use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2, pos2, vec2};

use crate::app::{Action, Env, Location, ObjRef};
use crate::backend::{Msg, TabId};
use crate::icons::{self, Db, G};
use crate::i18n;

const ROW: f32 = 18.0;
const HEAD: f32 = 24.0;
const WIDTH: f32 = 220.0;

pub struct ModelTab {
    pub id: TabId,
    pub loc: Location,
    tables: Option<Result<Vec<TableInfo>, String>>,
    loading: bool,
    /// Top-left corner of each box, in sheet coordinates.
    pos: HashMap<String, Pos2>,
    pan: Vec2,
    zoom: f32,
    selected: Option<String>,
    /// Show the column types.
    types: bool,
    /// Arrange the boxes at the next frame (all of them, or only the ones without a place).
    pending_layout: Option<bool>,
    /// Width of the sheet at the last frame (the layout fills it).
    width: f32,
}

/// Loads the tables of `loc` with their columns and keys.
fn load(env: &Env<'_>, tab: TabId, loc: &Location) {
    let Some(cfg) = env.cfg(&loc.conn).cloned() else {
        return;
    };
    let schema = loc.schema.clone();
    env.backend.with_driver(&cfg, &loc.db, move |d| async move {
        let res = async {
            let d = d?;
            let objs = d.list_objects(schema.as_deref()).await?;
            let mut out = Vec::new();
            for o in objs.iter().filter(|o| o.kind == ObjectKind::Table) {
                out.push(d.describe_table(schema.as_deref(), &o.name).await?);
            }
            Ok(out)
        }
        .await;
        Msg::Model { tab, res }
    });
}

fn box_height(t: &TableInfo) -> f32 {
    HEAD + t.columns.len().max(1) as f32 * ROW + 6.0
}

impl ModelTab {
    pub fn new(id: TabId, loc: Location) -> Self {
        ModelTab { id, loc, tables: None, loading: false, pos: HashMap::new(), pan: vec2(20.0, 20.0), zoom: 1.0, selected: None, types: true, pending_layout: None, width: 900.0 }
    }

    pub fn handle(&mut self, msg: Msg) {
        if let Msg::Model { res, .. } = msg {
            self.loading = false;
            let res = res.map_err(|e| e.to_string());
            if let Ok(t) = &res {
                // new tables get a place, the moved ones stay where they are
                if t.iter().any(|t| !self.pos.contains_key(&t.name)) {
                    self.pending_layout = Some(self.pending_layout.unwrap_or(false));
                }
            }
            self.tables = Some(res);
        }
    }

    /// Arranges the boxes: referenced tables first, in columns.
    fn layout(&mut self, tables: &[TableInfo], all: bool) {
        let mut order: Vec<&TableInfo> = tables.iter().collect();
        // tables referenced by many others go first (left), then by name
        let refs = |n: &str| tables.iter().flat_map(|t| &t.foreign_keys).filter(|f| f.ref_table == n).count();
        order.sort_by(|a, b| refs(&b.name).cmp(&refs(&a.name)).then(a.name.cmp(&b.name)));
        // as many columns as fit the sheet (at least the square root, to stay compact)
        let fit = ((self.width / self.zoom - 20.0) / (WIDTH + 70.0)).floor().max(1.0) as usize;
        let cols = fit.min(((order.len() as f32).sqrt().ceil() as usize).max(fit.min(order.len()))).max(1);
        let mut heights = vec![0.0f32; cols];
        for (i, t) in order.iter().enumerate() {
            if !all && self.pos.contains_key(&t.name) {
                continue;
            }
            let c = i % cols;
            let p = pos2(c as f32 * (WIDTH + 70.0), heights[c]);
            heights[c] += box_height(t) + 40.0;
            self.pos.insert(t.name.clone(), p);
        }
    }

    fn export_sql(&self, env: &mut Env<'_>) {
        let Some(Ok(tables)) = &self.tables else {
            return;
        };
        let dialect = env.dialect(&self.loc.conn);
        let mut sql = String::new();
        for t in tables {
            let mut d = TableDesign::from_info(t);
            d.schema = if dialect == Dialect::Sqlite { None } else { self.loc.schema.clone() };
            for s in dialect.create_table(&d) {
                sql.push_str(&s);
                sql.push_str(";\n");
            }
            sql.push('\n');
        }
        let name = format!("{}.sql", if self.loc.db.is_empty() { "modello" } else { &self.loc.db });
        if let Some(p) = rfd::FileDialog::new().add_filter("SQL", &["sql"]).set_file_name(name).save_file() {
            match std::fs::write(&p, sql) {
                Ok(()) => env.actions.push(Action::Notice(format!("Modello esportato in {}", p.display()), false)),
                Err(e) => env.actions.push(Action::Notice(e.to_string(), true)),
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        if self.tables.is_none() && !self.loading {
            self.loading = true;
            load(env, self.id, &self.loc);
        }
        ui.horizontal_wrapped(|ui| {
            if icons::button(ui, Db::Refresh, "Aggiorna").clicked() {
                self.loading = true;
                load(env, self.id, &self.loc);
            }
            if icons::button(ui, Db::Model, "Disponi").on_hover_text(i18n::t("Rimette in ordine i riquadri")).clicked()
                && let Some(Ok(t)) = &self.tables
            {
                let _ = t;
                self.pending_layout = Some(true);
                self.pan = vec2(20.0, 20.0);
            }
            ui.separator();
            if icons::small(ui, G::Minus, "Riduci", true).clicked() {
                self.zoom = (self.zoom / 1.2).max(0.3);
            }
            ui.label(i18n::tf(format!("{:.0}%", self.zoom * 100.0)));
            if icons::small(ui, G::Plus, "Ingrandisci", true).clicked() {
                self.zoom = (self.zoom * 1.2).min(2.5);
            }
            if ui.button("100%").clicked() {
                self.zoom = 1.0;
            }
            crate::theme::checkbox(ui, &mut self.types, i18n::t("Tipi"));
            ui.separator();
            if icons::button_ex(ui, Db::Export, "Esporta SQL", false, matches!(self.tables, Some(Ok(_)))).clicked() {
                self.export_sql(env);
            }
            if let Some(Ok(t)) = &self.tables {
                ui.label(RichText::new(i18n::tf(format!("{} tabelle", t.len()))).weak())
                    .on_hover_text(i18n::t("Trascina i riquadri per spostarli e lo sfondo per scorrere; ⌘/Ctrl + rotella per lo zoom; doppio clic su una tabella per progettarla"));
            }
            if let Some(name) = self.selected.clone() {
                ui.separator();
                if icons::button(ui, Db::Design, &format!("Progetta {name}")).clicked() {
                    env.actions.push(Action::DesignTable(self.obj(&name)));
                }
                if icons::button(ui, Db::Table, "Apri").clicked() {
                    env.actions.push(Action::OpenTable(self.obj(&name)));
                }
            }
        });
        ui.separator();
        if self.loading && self.tables.is_none() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(i18n::t("Leggo le tabelle…"));
            });
            return;
        }
        let tables = match &self.tables {
            Some(Ok(t)) => t.clone(),
            Some(Err(e)) => {
                ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
                return;
            }
            None => return,
        };
        if tables.is_empty() {
            ui.label(RichText::new(i18n::t("Nessuna tabella in questo database.")).weak());
            return;
        }
        self.canvas(ui, env, &tables);
    }

    fn obj(&self, name: &str) -> ObjRef {
        ObjRef { conn: self.loc.conn.clone(), db: self.loc.db.clone(), schema: self.loc.schema.clone(), name: name.to_string(), kind: ObjectKind::Table }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>, tables: &[TableInfo]) {
        let (area, bg) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        self.width = area.width();
        if let Some(all) = self.pending_layout.take() {
            self.layout(tables, all);
        }
        let painter = ui.painter_at(area);
        let v = ui.visuals().clone();
        let dark = v.dark_mode;
        painter.rect_filled(area, 0.0, if dark { Color32::from_gray(30) } else { Color32::from_rgb(0xF7, 0xF8, 0xFA) });
        // dotted grid
        let z = self.zoom;
        let step = 24.0 * z;
        if step > 8.0 {
            let dot = if dark { Color32::from_gray(55) } else { Color32::from_gray(215) };
            let ox = (area.left() + self.pan.x).rem_euclid(step);
            let oy = (area.top() + self.pan.y).rem_euclid(step);
            let mut y = area.top() + oy;
            while y < area.bottom() {
                let mut x = area.left() + ox;
                while x < area.right() {
                    painter.circle_filled(pos2(x, y), 0.9, dot);
                    x += step;
                }
                y += step;
            }
        }
        if bg.dragged() {
            self.pan += bg.drag_delta();
        }
        if bg.clicked() {
            self.selected = None;
        }
        // zoom with ⌘/Ctrl + wheel, pan with the wheel
        if bg.hovered() || ui.rect_contains_pointer(area) {
            let (scroll, zoom_delta, pointer) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            if zoom_delta != 1.0 {
                let old = self.zoom;
                self.zoom = (self.zoom * zoom_delta).clamp(0.3, 2.5);
                if let Some(p) = pointer {
                    let rel = p - area.min - self.pan;
                    self.pan -= rel * (self.zoom / old - 1.0);
                }
            } else {
                self.pan += scroll;
            }
        }
        let to_screen = |p: Pos2| area.min + self.pan + p.to_vec2() * z;
        let rect_of = |t: &TableInfo, pos: &HashMap<String, Pos2>| -> Rect {
            let p = pos.get(&t.name).copied().unwrap_or_default();
            Rect::from_min_size(to_screen(p), vec2(WIDTH, box_height(t)) * z)
        };
        // the foreign keys under the boxes
        let line = if dark { Color32::from_rgb(0x7A, 0x9C, 0xD6) } else { Color32::from_rgb(0x4A, 0x6F, 0xB5) };
        for t in tables {
            let a = rect_of(t, &self.pos);
            for f in &t.foreign_keys {
                let Some(target) = tables.iter().find(|x| x.name == f.ref_table) else {
                    continue;
                };
                let b = rect_of(target, &self.pos);
                let row_y = |r: &Rect, tab: &TableInfo, col: Option<&String>| {
                    let i = col.and_then(|c| tab.columns.iter().position(|x| &x.name == c)).unwrap_or(0);
                    r.top() + (HEAD + i as f32 * ROW + ROW / 2.0) * z
                };
                let ya = row_y(&a, t, f.columns.first());
                let yb = row_y(&b, target, f.ref_columns.first());
                let selected = self.selected.as_deref().is_some_and(|s| s == t.name || s == target.name);
                let stroke = Stroke::new(if selected { 2.2 } else { 1.3 }, if selected { v.selection.bg_fill } else { line });
                if std::ptr::eq(t, target) {
                    // self reference: a loop on the right
                    let x = a.right();
                    let pts = vec![pos2(x, ya), pos2(x + 24.0 * z, ya), pos2(x + 24.0 * z, yb + 10.0 * z), pos2(x, yb + 10.0 * z)];
                    painter.add(egui::Shape::line(pts, stroke));
                    continue;
                }
                // one above the other: both ends on the right, the line bulging out; side by side:
                // from the facing sides
                let stacked = (a.center().x - b.center().x).abs() < a.width() * 0.8;
                let (xa, xb, da, db) = if stacked {
                    (a.right(), b.right(), 1.0, 1.0)
                } else if b.center().x >= a.center().x {
                    (a.right(), b.left(), 1.0, -1.0)
                } else {
                    (a.left(), b.right(), -1.0, 1.0)
                };
                let k = if stacked { 30.0 * z + (ya - yb).abs() * 0.15 } else { 40.0 * z };
                let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                    [pos2(xa, ya), pos2(xa + da * k, ya), pos2(xb + db * k, yb), pos2(xb, yb)],
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                );
                painter.add(curve);
                // crow's foot on the "many" side (the referencing table), a bar on the "one" side
                let s = 7.0 * z;
                for dy in [-s * 0.7, 0.0, s * 0.7] {
                    painter.line_segment([pos2(xa + da * s, ya), pos2(xa, ya + dy)], stroke);
                }
                painter.line_segment([pos2(xb + db * s * 0.8, yb - s * 0.7), pos2(xb + db * s * 0.8, yb + s * 0.7)], stroke);
            }
        }
        // the boxes
        let font = egui::FontId::proportional(12.0 * z);
        let small = egui::FontId::proportional(11.0 * z);
        let head_fill = Color32::from_rgb(0x2F, 0x6F, 0xD8);
        let paper = if dark { Color32::from_gray(45) } else { Color32::WHITE };
        let edge = if dark { Color32::from_gray(80) } else { Color32::from_gray(190) };
        let mut open = None;
        for t in tables {
            let r = rect_of(t, &self.pos);
            if !area.intersects(r) {
                continue;
            }
            let id = ui.scope_id().with(("model_box", self.id, &t.name));
            let resp = ui.interact(r, id, egui::Sense::click_and_drag());
            if resp.dragged() {
                if let Some(p) = self.pos.get_mut(&t.name) {
                    *p += resp.drag_delta() / z;
                }
                self.selected = Some(t.name.clone());
            }
            if resp.clicked() {
                self.selected = Some(t.name.clone());
            }
            if resp.double_clicked() {
                open = Some(t.name.clone());
            }
            let selected = self.selected.as_deref() == Some(t.name.as_str());
            painter.rect_filled(r.translate(vec2(2.0, 3.0)), 5.0 * z, Color32::from_black_alpha(if dark { 70 } else { 25 }));
            painter.rect_filled(r, 5.0 * z, paper);
            let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD * z));
            painter.rect_filled(head, egui::CornerRadius { nw: (5.0 * z) as u8, ne: (5.0 * z) as u8, sw: 0, se: 0 }, head_fill);
            icons::paint(&painter, Rect::from_center_size(pos2(head.left() + 13.0 * z, head.center().y), Vec2::splat(14.0 * z)), Db::Table, Color32::WHITE);
            painter.text(pos2(head.left() + 24.0 * z, head.center().y), egui::Align2::LEFT_CENTER, &t.name, egui::FontId::proportional(12.5 * z), Color32::WHITE);
            let fk_cols: Vec<&String> = t.foreign_keys.iter().flat_map(|f| &f.columns).collect();
            for (i, c) in t.columns.iter().enumerate() {
                let y = r.top() + (HEAD + i as f32 * ROW + ROW / 2.0) * z;
                let ic = Rect::from_center_size(pos2(r.left() + 12.0 * z, y), Vec2::splat(12.0 * z));
                if c.primary_key {
                    icons::paint(&painter, ic, Db::Key, v.text_color());
                } else if fk_cols.contains(&&c.name) {
                    icons::paint(&painter, ic, Db::ForeignKey, v.text_color());
                } else {
                    painter.circle_stroke(ic.center(), 2.2 * z, Stroke::new(1.0, v.weak_text_color()));
                }
                let name_color = if c.primary_key { v.strong_text_color() } else { v.text_color() };
                painter.text(pos2(r.left() + 24.0 * z, y), egui::Align2::LEFT_CENTER, &c.name, font.clone(), name_color);
                if self.types {
                    let ty: String = c.data_type.chars().take(22).collect();
                    painter.text(pos2(r.right() - 8.0 * z, y), egui::Align2::RIGHT_CENTER, ty, small.clone(), v.weak_text_color());
                }
            }
            painter.rect_stroke(r, 5.0 * z, Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { v.selection.bg_fill } else { edge }), egui::StrokeKind::Inside);
            resp.on_hover_text(i18n::tf(format!("{} colonne, {} chiavi esterne\nDoppio clic: progetta la tabella", t.columns.len(), t.foreign_keys.len())));
        }
        if let Some(name) = open {
            env.actions.push(Action::DesignTable(self.obj(&name)));
        }
    }
}
