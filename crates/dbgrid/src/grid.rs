//! The data grid widget: virtualised painting of only the visible cells, a frozen-column pane,
//! column headers with type and key markers, a row-number gutter, selection, column resizing,
//! keyboard navigation, the in-cell editor and a context menu.
//!
//! Painting order and the drag model follow GridCraft's worksheet grid (MIT OR Apache-2.0);
//! see NOTICE.md.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Align2, Color32, CursorIcon, FontId, Id, Key, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use crate::datetime::{DateKind, PickerOutcome};
use crate::geo::Geo;
use crate::line_index::LineIndex;
use crate::model::{ColumnKind, GridModel, RowState};
use crate::theme::GridTheme;

const HEADER_H: f32 = 34.0;
const SCROLLBAR: f32 = 12.0;
const MIN_COL_W: f32 = 30.0;
const RESIZE_GRAB: f32 = 4.0;
/// Longest text painted in a cell; the rest is cut (huge TEXT/BLOB values).
const MAX_CELL_CHARS: usize = 300;
/// Cap on cells copied to the clipboard in one go.
const MAX_COPY_CELLS: usize = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

/// What the user asked for this frame. The grid never mutates data itself.
#[derive(Clone, Debug, PartialEq)]
pub enum GridEvent {
    CellEdited { row: usize, col: usize, value: Option<String> },
    SortRequested { col: usize, dir: Option<SortDir> },
    FilterByValue { col: usize, value: Option<String> },
    DeleteRows(Vec<usize>),
    InsertRow,
    /// Double-click on a read-only grid.
    Activated { row: usize, col: usize },
    SelectionChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelKind {
    #[default]
    Cells,
    Rows,
    Cols,
}

/// Rectangular selection: `anchor` is where it started, `active` is the cursor cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: (u32, u32),
    pub active: (u32, u32),
    pub kind: SelKind,
}

impl Selection {
    /// Inclusive (row0, col0, row1, col1) clamped to the given size.
    pub fn bounds(&self, rows: u32, cols: u32) -> Option<(u32, u32, u32, u32)> {
        if rows == 0 || cols == 0 {
            return None;
        }
        let (mut r0, mut r1) = (self.anchor.0.min(self.active.0), self.anchor.0.max(self.active.0));
        let (mut c0, mut c1) = (self.anchor.1.min(self.active.1), self.anchor.1.max(self.active.1));
        match self.kind {
            SelKind::Rows => (c0, c1) = (0, cols - 1),
            SelKind::Cols => (r0, r1) = (0, rows - 1),
            SelKind::Cells => {}
        }
        Some((r0.min(rows - 1), c0.min(cols - 1), r1.min(rows - 1), c1.min(cols - 1)))
    }
    pub fn rows(&self, rows: u32, cols: u32) -> Vec<usize> {
        self.bounds(rows, cols.max(1)).map(|(r0, _, r1, _)| (r0 as usize..=r1 as usize).collect()).unwrap_or_default()
    }
    fn contains(&self, r: u32, c: u32, rows: u32, cols: u32) -> bool {
        self.bounds(rows, cols).is_some_and(|(r0, c0, r1, c1)| (r0..=r1).contains(&r) && (c0..=c1).contains(&c))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Drag {
    #[default]
    None,
    Select,
    Rows,
    Cols,
    ResizeCol {
        col: u32,
        start: f32,
        orig: f32,
    },
    VScroll {
        grab: f32,
    },
    HScroll {
        grab: f32,
    },
}

struct EditState {
    row: u32,
    col: u32,
    text: String,
    was_null: bool,
    focus: bool,
}

pub struct DataGrid {
    id: Id,
    pub scroll: egui::Vec2,
    pub sel: Selection,
    pub frozen_cols: u32,
    pub row_height: f32,
    /// Sort indicator shown in the header (the app owns the actual ordering).
    pub sort: Option<(usize, SortDir)>,
    widths: BTreeMap<u32, f32>,
    columns_sig: u64,
    drag: Drag,
    edit: Option<EditState>,
    /// Cell whose date/time picker is open.
    picker: Option<(u32, u32)>,
    ensure_visible: bool,
}

impl DataGrid {
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        DataGrid {
            id: Id::unique(("dbgrid", id_salt)),
            scroll: egui::Vec2::ZERO,
            sel: Selection::default(),
            frozen_cols: 0,
            row_height: 22.0,
            sort: None,
            widths: BTreeMap::new(),
            columns_sig: 0,
            drag: Drag::None,
            edit: None,
            picker: None,
            ensure_visible: false,
        }
    }

    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }

    /// Forget column widths and scroll (e.g. a new result set).
    pub fn reset(&mut self) {
        self.widths.clear();
        self.columns_sig = 0;
        self.scroll = egui::Vec2::ZERO;
        self.sel = Selection::default();
        self.edit = None;
        self.drag = Drag::None;
    }

    /// Opens the date/time picker on a cell (of a date-like column).
    pub fn open_date_picker(&mut self, row: u32, col: u32) {
        self.select_cell(row, col);
        self.picker = Some((row, col));
    }

    pub fn select_cell(&mut self, row: u32, col: u32) {
        self.sel = Selection { anchor: (row, col), active: (row, col), kind: SelKind::Cells };
        self.ensure_visible = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, model: &dyn GridModel) -> Vec<GridEvent> {
        let mut events = Vec::new();
        let theme = GridTheme::get(ui.ctx());
        let font = FontId::proportional(13.0);
        let small = FontId::proportional(10.5);
        let ncols = u32::try_from(model.columns().len()).unwrap_or(u32::MAX);
        let nrows = u32::try_from(model.row_count()).unwrap_or(u32::MAX);
        self.sync_widths(ui, model, &font);

        let (rect, _) = ui.allocate_exact_size(ui.available_size().max(vec2(100.0, 60.0)), Sense::hover());
        let resp = ui.interact(rect, self.id, Sense::click_and_drag());
        let resp = resp.on_hover_cursor(CursorIcon::Default);
        let gutter_w = gutter_width(nrows);
        let cells = Rect::from_min_max(pos2(rect.left() + gutter_w, rect.top() + HEADER_H), pos2(rect.right() - SCROLLBAR, rect.bottom() - SCROLLBAR));
        let ri = Arc::new(LineIndex::uniform(self.row_height, nrows));
        let ci = Arc::new(LineIndex::new(&self.widths, 100.0, ncols));
        let mut geo = Geo::new(rect, cells, self.frozen_cols, self.scroll, ri, ci);

        if resp.clicked() || resp.secondary_clicked() || resp.double_clicked() {
            resp.request_focus();
        }
        if resp.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(self.id, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false })
            });
        }

        // Wheel / trackpad scrolling.
        if resp.hovered() {
            let d = ui.input(|i| i.smooth_scroll_delta);
            if d != egui::Vec2::ZERO {
                self.scroll -= d;
            }
        }
        let before = self.sel;
        self.pointer(ui, &resp, &mut geo, model, &mut events);
        if resp.has_focus() && self.edit.is_none() {
            self.keyboard(ui, &geo, model, &mut events);
        }
        if self.ensure_visible {
            self.ensure_visible = false;
            self.scroll_to_active(&geo);
        }
        let max = geo.max_scroll();
        self.scroll = self.scroll.clamp(egui::Vec2::ZERO, max);
        geo.scroll = self.scroll;
        if self.sel != before {
            events.push(GridEvent::SelectionChanged);
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme.grid_bg);
        self.paint_cells(&painter, &geo, model, &theme, &font);
        self.paint_headers(&painter, &geo, model, &theme, &font, &small);
        self.paint_gutter(&painter, &geo, model, &theme, &small);
        paint_scrollbars(&painter, &geo, &theme);

        self.context_menu(ui, &resp, &geo, model, &mut events);
        self.editor(ui, &geo, model, &mut events);
        self.date_picker(ui, &geo, model, &mut events);
        events
    }

    /// Default widths: header text or a sample of the first rows, whichever is wider.
    fn sync_widths(&mut self, ui: &egui::Ui, model: &dyn GridModel, font: &FontId) {
        let sig = model.columns().iter().fold(model.columns().len() as u64, |h, c| {
            c.name.bytes().fold(h.wrapping_mul(31), |h, b| h.wrapping_mul(131).wrapping_add(u64::from(b)))
        });
        if sig == self.columns_sig {
            return;
        }
        self.columns_sig = sig;
        self.widths.clear();
        let painter = ui.painter();
        let sample = model.row_count().min(50);
        for (i, c) in model.columns().iter().enumerate() {
            let w = match c.width {
                Some(w) => w,
                None => {
                    let mut w = painter.layout_no_wrap(c.name.clone(), font.clone(), Color32::BLACK).size().x + 34.0;
                    w = w.max(painter.layout_no_wrap(c.type_name.clone(), FontId::proportional(10.5), Color32::BLACK).size().x + 44.0);
                    for r in 0..sample {
                        let t = model.cell(r, i).text;
                        let t: String = t.chars().take(60).collect();
                        w = w.max(painter.layout_no_wrap(t, font.clone(), Color32::BLACK).size().x + 14.0);
                    }
                    w.clamp(60.0, 320.0)
                }
            };
            self.widths.insert(i as u32, w);
        }
    }

    fn pointer(&mut self, ui: &egui::Ui, resp: &egui::Response, geo: &mut Geo, model: &dyn GridModel, events: &mut Vec<GridEvent>) {
        let (pos, pressed, down, shift) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_pressed(), i.pointer.primary_down(), i.modifiers.shift));
        let Some(p) = pos else { return };
        let (nrows, ncols) = (geo.rows(), geo.cols());
        let header = Rect::from_min_max(pos2(geo.cells.left(), geo.rect.top()), pos2(geo.cells.right(), geo.cells.top()));
        let gutter = Rect::from_min_max(pos2(geo.rect.left(), geo.cells.top()), pos2(geo.cells.left(), geo.cells.bottom()));
        let vbar = Rect::from_min_max(pos2(geo.cells.right(), geo.cells.top()), pos2(geo.rect.right(), geo.cells.bottom()));
        let hbar = Rect::from_min_max(pos2(geo.cells.left(), geo.cells.bottom()), pos2(geo.cells.right(), geo.rect.bottom()));
        let resize_col = || -> Option<u32> {
            if !header.contains(p) || ncols == 0 {
                return None;
            }
            let c = geo.col_at(p.x);
            [c.checked_sub(1), Some(c)].into_iter().flatten().find(|&c| (geo.x_end(c) - p.x).abs() <= RESIZE_GRAB)
        };

        if resp.hovered() && self.drag == Drag::None && resize_col().is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
        }

        // Double-click on a header boundary: fit the column to its content.
        if resp.double_clicked() && let Some(c) = resize_col() {
            let w = self.fit_width(ui, geo, model, c);
            self.widths.insert(c, w);
            self.drag = Drag::None;
            return;
        }

        if pressed && resp.hovered() {
            self.commit_edit(model, events);
            if let Some(col) = resize_col() {
                self.drag = Drag::ResizeCol { col, start: p.x, orig: geo.ci.size(col) as f32 };
            } else if vbar.contains(p) {
                let (thumb, _) = vthumb(geo);
                self.drag = Drag::VScroll { grab: if thumb.contains(p) { p.y - thumb.top() } else { thumb.height() / 2.0 } };
            } else if hbar.contains(p) {
                let (thumb, _) = hthumb(geo);
                self.drag = Drag::HScroll { grab: if thumb.contains(p) { p.x - thumb.left() } else { thumb.width() / 2.0 } };
            } else if header.contains(p) && ncols > 0 {
                let c = geo.col_at(p.x);
                let anchor = if shift && self.sel.kind == SelKind::Cols { self.sel.anchor } else { (0, c) };
                self.sel = Selection { anchor, active: (self.sel.active.0, c), kind: SelKind::Cols };
                self.drag = Drag::Cols;
            } else if gutter.contains(p) && nrows > 0 {
                let r = geo.row_at(p.y);
                let anchor = if shift && self.sel.kind == SelKind::Rows { self.sel.anchor } else { (r, 0) };
                self.sel = Selection { anchor, active: (r, self.sel.active.1), kind: SelKind::Rows };
                self.drag = Drag::Rows;
            } else if geo.in_cells(p) {
                let (r, c) = (geo.row_at(p.y), geo.col_at(p.x));
                let anchor = if shift { self.sel.anchor } else { (r, c) };
                self.sel = Selection { anchor, active: (r, c), kind: SelKind::Cells };
                self.drag = Drag::Select;
            }
        } else if down {
            match self.drag.clone() {
                Drag::ResizeCol { col, start, orig } => {
                    self.widths.insert(col, (orig + p.x - start).max(MIN_COL_W));
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
                }
                Drag::VScroll { grab } => {
                    let (_, track) = vthumb(geo);
                    let (thumb, _) = vthumb(geo);
                    let free = (track.height() - thumb.height()).max(1.0);
                    self.scroll.y = ((p.y - grab - track.top()) / free).clamp(0.0, 1.0) * geo.max_scroll().y;
                }
                Drag::HScroll { grab } => {
                    let (thumb, track) = hthumb(geo);
                    let free = (track.width() - thumb.width()).max(1.0);
                    self.scroll.x = ((p.x - grab - track.left()) / free).clamp(0.0, 1.0) * geo.max_scroll().x;
                }
                Drag::Select if nrows > 0 && ncols > 0 => {
                    self.autoscroll(p, geo);
                    self.sel.active = (geo.row_at(p.y.clamp(geo.cells.top(), geo.cells.bottom() - 1.0)), geo.col_at(p.x.clamp(geo.cells.left(), geo.cells.right() - 1.0)));
                }
                Drag::Rows if nrows > 0 => {
                    self.autoscroll(p, geo);
                    self.sel.active.0 = geo.row_at(p.y.clamp(geo.cells.top(), geo.cells.bottom() - 1.0));
                }
                Drag::Cols if ncols > 0 => {
                    self.autoscroll(p, geo);
                    self.sel.active.1 = geo.col_at(p.x.clamp(geo.cells.left(), geo.cells.right() - 1.0));
                }
                _ => {}
            }
        } else {
            self.drag = Drag::None;
        }

        if resp.double_clicked() && geo.in_cells(p) && nrows > 0 {
            let (r, c) = (geo.row_at(p.y), geo.col_at(p.x));
            if model.editable() {
                self.start_edit(model, r, c, None);
            } else {
                events.push(GridEvent::Activated { row: r as usize, col: c as usize });
            }
        }
        // Right click outside the selection moves it (so the menu acts on that cell).
        if resp.secondary_clicked() && geo.in_cells(p) {
            let (r, c) = (geo.row_at(p.y), geo.col_at(p.x));
            if !self.sel.contains(r, c, nrows, ncols) {
                self.select_cell(r, c);
            }
        }
    }

    fn autoscroll(&mut self, p: Pos2, geo: &Geo) {
        let c = geo.cells;
        let step = |d: f32| (d / 4.0).clamp(-40.0, 40.0);
        if p.y > c.bottom() {
            self.scroll.y += step(p.y - c.bottom()).max(2.0);
        } else if p.y < c.top() && self.drag != Drag::Cols {
            self.scroll.y += step(p.y - c.top()).min(-2.0);
        }
        if p.x > c.right() {
            self.scroll.x += step(p.x - c.right()).max(2.0);
        } else if p.x < c.left() + geo.frozen_w && self.drag != Drag::Rows {
            self.scroll.x += step(p.x - c.left() - geo.frozen_w).min(-2.0);
        }
    }

    fn fit_width(&self, ui: &egui::Ui, geo: &Geo, model: &dyn GridModel, c: u32) -> f32 {
        let font = FontId::proportional(13.0);
        let painter = ui.painter();
        let mut w = model.columns().get(c as usize).map_or(60.0, |col| painter.layout_no_wrap(col.name.clone(), font.clone(), Color32::BLACK).size().x + 34.0);
        if let Some((r0, r1)) = geo.visible_rows() {
            for r in r0..=r1.min(r0 + 200) {
                let t: String = model.cell(r as usize, c as usize).text.chars().take(MAX_CELL_CHARS).collect();
                w = w.max(painter.layout_no_wrap(t, font.clone(), Color32::BLACK).size().x + 14.0);
            }
        }
        w.clamp(MIN_COL_W, 800.0)
    }

    fn date_kind(&self, model: &dyn GridModel, col: u32) -> Option<DateKind> {
        model.columns().get(col as usize).and_then(|c| DateKind::of(c.kind))
    }

    /// The calendar button on the active date/time cell and its picker popup.
    fn date_picker(&mut self, ui: &mut egui::Ui, geo: &Geo, model: &dyn GridModel, events: &mut Vec<GridEvent>) {
        let (r, c) = self.sel.active;
        let editable = model.editable();
        // Alt+↓ while typing in the cell opens the picker too.
        if editable && self.edit.as_ref().is_some_and(|e| e.row == r && e.col == c) && ui.input(|i| i.modifiers.alt && i.key_pressed(Key::ArrowDown)) {
            self.picker = Some((r, c));
        }
        if editable
            && self.picker.is_none()
            && (r as usize) < model.row_count()
            && self.date_kind(model, c).is_some()
        {
            let cell = geo.cell_rect(r, c);
            let side = (cell.height() - 4.0).max(10.0);
            let btn = Rect::from_min_size(pos2(cell.right() - side - 2.0, cell.top() + 2.0), vec2(side, side));
            if geo.cells.contains_rect(btn) && (c < geo.fc || btn.left() >= geo.cells.left() + geo.frozen_w) {
                let resp = ui.interact(btn, self.id.with("date-btn"), Sense::click()).on_hover_text("Scegli data/ora (Alt+↓)");
                let theme = GridTheme::get(ui.ctx());
                let p = ui.painter_at(geo.cells);
                p.rect_filled(btn, 3.0, if resp.hovered() { theme.header_sel_bg } else { theme.grid_bg });
                p.rect_stroke(btn, 3.0, Stroke::new(1.0, theme.header_line), StrokeKind::Inside);
                crate::datetime::paint_calendar_glyph(&p, btn, theme.sel_border);
                if resp.clicked() {
                    self.picker = Some((r, c));
                }
            }
        }
        let Some((pr, pc)) = self.picker else { return };
        let Some(kind) = self.date_kind(model, pc) else {
            self.picker = None;
            return;
        };
        let current = match &self.edit {
            Some(e) if e.row == pr && e.col == pc => e.text.clone(),
            _ => {
                let v = model.cell(pr as usize, pc as usize);
                if v.null { String::new() } else { v.text.into_owned() }
            }
        };
        let anchor = geo.cell_rect(pr, pc).intersect(geo.cells);
        match crate::datetime::popup(ui.ctx(), self.id.with(("picker", pr, pc)), anchor, kind, &current, true) {
            Some(PickerOutcome::Pick(text)) => {
                self.edit = None;
                self.picker = None;
                events.push(GridEvent::CellEdited { row: pr as usize, col: pc as usize, value: Some(text) });
            }
            Some(PickerOutcome::Null) => {
                self.edit = None;
                self.picker = None;
                events.push(GridEvent::CellEdited { row: pr as usize, col: pc as usize, value: None });
            }
            Some(PickerOutcome::Cancel) => self.picker = None,
            None => {}
        }
    }

    fn keyboard(&mut self, ui: &egui::Ui, geo: &Geo, model: &dyn GridModel, events: &mut Vec<GridEvent>) {
        let (nrows, ncols) = (geo.rows(), geo.cols());
        if nrows == 0 || ncols == 0 {
            return;
        }
        let page = ((geo.cells.height() / self.row_height).floor() as u32).max(1);
        let (mut keys, mods, text, copy) = ui.input(|i| {
            let keys: Vec<Key> = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, .. } => Some(*key),
                    _ => None,
                })
                .collect();
            let text: String = i
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect();
            let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy));
            (keys, i.modifiers, text, copy)
        });
        if copy {
            ui.ctx().copy_text(self.selection_tsv(model, nrows, ncols));
        }
        let (mut r, mut c) = self.sel.active;
        let mut moved = false;
        for key in keys.drain(..) {
            let before = (r, c);
            match key {
                Key::ArrowDown if mods.alt => {
                    if model.editable() && self.date_kind(model, c).is_some() {
                        self.picker = Some((r, c));
                    }
                    return;
                }
                Key::ArrowDown => r = if mods.command { nrows - 1 } else { (r + 1).min(nrows - 1) },
                Key::ArrowUp => r = if mods.command { 0 } else { r.saturating_sub(1) },
                Key::ArrowRight => c = if mods.command { ncols - 1 } else { (c + 1).min(ncols - 1) },
                Key::ArrowLeft => c = if mods.command { 0 } else { c.saturating_sub(1) },
                Key::Tab if mods.shift => c = c.saturating_sub(1),
                Key::Tab => c = (c + 1).min(ncols - 1),
                Key::PageDown => r = (r + page).min(nrows - 1),
                Key::PageUp => r = r.saturating_sub(page),
                Key::Home if mods.command => (r, c) = (0, 0),
                Key::Home => c = 0,
                Key::End if mods.command => (r, c) = (nrows - 1, ncols - 1),
                Key::End => c = ncols - 1,
                Key::A if mods.command => {
                    self.sel = Selection { anchor: (0, 0), active: (nrows - 1, ncols - 1), kind: SelKind::Cells };
                    return;
                }
                Key::Enter | Key::F2 if model.editable() => {
                    self.start_edit(model, r, c, None);
                    return;
                }
                Key::Delete | Key::Backspace if mods.command && model.editable() => {
                    events.push(GridEvent::DeleteRows(self.sel.rows(nrows, ncols)));
                    return;
                }
                _ => {}
            }
            moved |= before != (r, c);
        }
        if moved {
            let extend = mods.shift && self.sel.kind == SelKind::Cells;
            self.sel = Selection { anchor: if extend { self.sel.anchor } else { (r, c) }, active: (r, c), kind: SelKind::Cells };
            self.ensure_visible = true;
        } else if !text.is_empty() && !mods.command && model.editable() {
            self.start_edit(model, r, c, Some(text));
        }
    }

    fn scroll_to_active(&mut self, geo: &Geo) {
        let (r, c) = self.sel.active;
        if r < geo.rows() {
            let top = geo.ri.start(r) as f32;
            let bottom = top + self.row_height;
            if top < self.scroll.y {
                self.scroll.y = top;
            } else if bottom > self.scroll.y + geo.cells.height() {
                self.scroll.y = bottom - geo.cells.height();
            }
        }
        if c >= geo.fc && c < geo.cols() {
            let left = (geo.ci.start(c) - geo.ci.start(geo.fc)) as f32;
            let right = left + geo.ci.size(c) as f32;
            let view = geo.cells.width() - geo.frozen_w;
            if left < self.scroll.x {
                self.scroll.x = left;
            } else if right > self.scroll.x + view {
                self.scroll.x = (right - view).min(left);
            }
        }
    }

    pub fn selection_tsv(&self, model: &dyn GridModel, nrows: u32, ncols: u32) -> String {
        let Some((r0, c0, r1, c1)) = self.sel.bounds(nrows, ncols) else { return String::new() };
        let width = (c1 - c0 + 1) as usize;
        let max_rows = (MAX_COPY_CELLS / width.max(1)).max(1) as u32;
        let mut out = String::new();
        for r in r0..=r1.min(r0.saturating_add(max_rows - 1)) {
            for c in c0..=c1 {
                if c > c0 {
                    out.push('\t');
                }
                let v = model.cell(r as usize, c as usize);
                if !v.null {
                    out.push_str(&v.text.replace(['\t', '\n', '\r'], " "));
                }
            }
            out.push('\n');
        }
        out
    }

    fn start_edit(&mut self, model: &dyn GridModel, row: u32, col: u32, typed: Option<String>) {
        if !model.editable() || row as usize >= model.row_count() || col as usize >= model.columns().len() {
            return;
        }
        let cell = model.cell(row as usize, col as usize);
        let was_null = cell.null;
        let text = typed.unwrap_or_else(|| if cell.null { String::new() } else { cell.text.into_owned() });
        self.sel = Selection { anchor: (row, col), active: (row, col), kind: SelKind::Cells };
        self.edit = Some(EditState { row, col, text, was_null, focus: true });
        self.ensure_visible = true;
    }

    fn commit_edit(&mut self, model: &dyn GridModel, events: &mut Vec<GridEvent>) {
        let Some(e) = self.edit.take() else { return };
        let cur = model.cell(e.row as usize, e.col as usize);
        let unchanged = if e.was_null { e.text.is_empty() } else { !cur.null && cur.text == e.text };
        if !unchanged {
            events.push(GridEvent::CellEdited { row: e.row as usize, col: e.col as usize, value: Some(e.text) });
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, geo: &Geo, model: &dyn GridModel, events: &mut Vec<GridEvent>) {
        let Some(e) = self.edit.as_mut() else { return };
        let rect = geo.cell_rect(e.row, e.col).intersect(geo.cells);
        if rect.width() < 4.0 || rect.height() < 4.0 {
            return;
        }
        let edit_id = self.id.with("editor");
        let out = ui.put(rect, egui::TextEdit::singleline(&mut e.text).id(edit_id).margin(vec2(3.0, 2.0)).font(FontId::proportional(13.0)));
        if e.focus {
            out.request_focus();
            e.focus = false;
        }
        let (enter, esc, tab) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape), i.key_pressed(Key::Tab)));
        if esc {
            self.edit = None;
            ui.memory_mut(|m| m.request_focus(self.id));
        } else if enter || tab || out.lost_focus() {
            let (r, c) = (e.row, e.col);
            self.commit_edit(model, events);
            if enter || tab {
                let next = if enter { ((r + 1).min(geo.rows().saturating_sub(1)), c) } else { (r, (c + 1).min(geo.cols().saturating_sub(1))) };
                self.select_cell(next.0, next.1);
            }
        }
    }

    fn context_menu(&mut self, _ui: &egui::Ui, resp: &egui::Response, geo: &Geo, model: &dyn GridModel, events: &mut Vec<GridEvent>) {
        let (nrows, ncols) = (geo.rows(), geo.cols());
        let (r, c) = self.sel.active;
        let editable = model.editable();
        resp.context_menu(|ui| {
            if ui.button("Copia").clicked() {
                ui.ctx().copy_text(self.selection_tsv(model, nrows, ncols));
                ui.close();
            }
            if ui.button("Copia con intestazioni").clicked() {
                let mut s = String::new();
                if let Some((_, c0, _, c1)) = self.sel.bounds(nrows, ncols) {
                    let names: Vec<&str> = (c0..=c1).filter_map(|c| model.columns().get(c as usize).map(|x| x.name.as_str())).collect();
                    s = names.join("\t") + "\n";
                }
                s.push_str(&self.selection_tsv(model, nrows, ncols));
                ui.ctx().copy_text(s);
                ui.close();
            }
            ui.separator();
            if editable && nrows > 0 {
                if ui.button("Imposta a NULL").clicked() {
                    self.edit = None;
                    if let Some((r0, c0, r1, c1)) = self.sel.bounds(nrows, ncols) {
                        for row in r0..=r1.min(r0 + 10_000) {
                            for col in c0..=c1 {
                                events.push(GridEvent::CellEdited { row: row as usize, col: col as usize, value: None });
                            }
                        }
                    }
                    ui.close();
                }
                if ui.button("Imposta a stringa vuota").clicked() {
                    events.push(GridEvent::CellEdited { row: r as usize, col: c as usize, value: Some(String::new()) });
                    ui.close();
                }
                ui.separator();
                if ui.button("Aggiungi record").clicked() {
                    events.push(GridEvent::InsertRow);
                    ui.close();
                }
                if ui.button("Elimina record").clicked() {
                    events.push(GridEvent::DeleteRows(self.sel.rows(nrows, ncols)));
                    ui.close();
                }
                ui.separator();
            }
            if (c as usize) < model.columns().len() {
                if ui.button("Ordina crescente").clicked() {
                    events.push(GridEvent::SortRequested { col: c as usize, dir: Some(SortDir::Asc) });
                    ui.close();
                }
                if ui.button("Ordina decrescente").clicked() {
                    events.push(GridEvent::SortRequested { col: c as usize, dir: Some(SortDir::Desc) });
                    ui.close();
                }
                if self.sort.is_some() && ui.button("Rimuovi ordinamento").clicked() {
                    events.push(GridEvent::SortRequested { col: c as usize, dir: None });
                    ui.close();
                }
                if nrows > 0 && ui.button("Filtra per questo valore").clicked() {
                    let v = model.cell(r as usize, c as usize);
                    events.push(GridEvent::FilterByValue { col: c as usize, value: (!v.null).then(|| v.text.into_owned()) });
                    ui.close();
                }
                ui.separator();
            }
            let label = if self.frozen_cols > 0 { "Sblocca colonne" } else { "Blocca fino a questa colonna" };
            if ui.button(label).clicked() {
                self.frozen_cols = if self.frozen_cols > 0 { 0 } else { c + 1 };
                ui.close();
            }
        });
    }

    fn paint_cells(&self, painter: &Painter, geo: &Geo, model: &dyn GridModel, theme: &GridTheme, font: &FontId) {
        let Some((r0, r1)) = geo.visible_rows() else { return };
        let (nrows, ncols) = (geo.rows(), geo.cols());
        let split = geo.cells.left() + geo.frozen_w;
        let frozen_clip = Rect::from_min_max(geo.cells.min, pos2(split, geo.cells.bottom()));
        let scroll_clip = Rect::from_min_max(pos2(split, geo.cells.top()), geo.cells.max);
        let mut panes: Vec<(Rect, Vec<u32>)> = Vec::with_capacity(2);
        if let Some((c0, c1)) = geo.visible_cols() {
            panes.push((scroll_clip, (c0..=c1).collect()));
        }
        if geo.fc > 0 {
            panes.push((frozen_clip, (0..geo.fc).collect()));
        }
        let sel = self.sel.bounds(nrows, ncols);
        for (clip, cols) in &panes {
            let p = painter.with_clip_rect(*clip);
            for r in r0..=r1 {
                let (y0, y1) = (geo.y(r), geo.y_end(r));
                let state = model.row_state(r as usize);
                let row_fill = match state {
                    RowState::Inserted => Some(theme.inserted_fill),
                    RowState::Deleted => Some(theme.deleted_fill),
                    _ if r % 2 == 1 => Some(theme.alt_row),
                    _ => None,
                };
                if let Some(f) = row_fill {
                    p.rect_filled(Rect::from_min_max(pos2(clip.left(), y0), pos2(clip.right(), y1)), 0.0, f);
                }
                if r == self.sel.active.0 && self.sel.kind == SelKind::Cells {
                    p.rect_filled(Rect::from_min_max(pos2(clip.left(), y0), pos2(clip.right(), y1)), 0.0, theme.active_row);
                }
                for &c in cols {
                    let rect = geo.cell_rect(r, c);
                    let cell = model.cell(r as usize, c as usize);
                    if cell.dirty {
                        p.rect_filled(rect, 0.0, theme.dirty_fill);
                    }
                    if sel.is_some_and(|(sr0, sc0, sr1, sc1)| (sr0..=sr1).contains(&r) && (sc0..=sc1).contains(&c)) {
                        p.rect_filled(rect, 0.0, theme.sel_fill);
                    }
                    let kind = model.columns().get(c as usize).map(|x| x.kind).unwrap_or_default();
                    let right = matches!(kind, ColumnKind::Number) && !cell.null;
                    let text = first_line(&cell.text);
                    let color = if cell.null { theme.null_text } else { theme.cell_text };
                    let inner = rect.shrink2(vec2(4.0, 0.0));
                    let cp = p.with_clip_rect(inner.intersect(*clip));
                    if right {
                        cp.text(pos2(inner.right(), inner.center().y), Align2::RIGHT_CENTER, text, font.clone(), color);
                    } else {
                        cp.text(pos2(inner.left(), inner.center().y), Align2::LEFT_CENTER, text, font.clone(), color);
                    }
                    if state == RowState::Deleted {
                        cp.hline(inner.x_range(), inner.center().y, Stroke::new(1.0, theme.null_text));
                    }
                }
            }
            // Gridlines.
            let stroke = Stroke::new(1.0, theme.gridline);
            let bottom = geo.y_end(r1).min(geo.cells.bottom());
            for &c in cols {
                p.vline(geo.x_end(c) - 0.5, geo.cells.top()..=bottom, stroke);
            }
            let right = cols.last().map_or(clip.left(), |&c| geo.x_end(c)).min(clip.right());
            for r in r0..=r1 {
                p.hline(clip.left()..=right, geo.y_end(r) - 0.5, stroke);
            }
        }
        if geo.fc > 0 {
            painter.vline(split, geo.cells.y_range(), Stroke::new(1.5, theme.header_line));
        }
        // Active cell border.
        let (ar, ac) = self.sel.active;
        if ar < nrows && ac < ncols && self.edit.is_none() {
            let clip = if ac < geo.fc { frozen_clip } else { scroll_clip };
            painter.with_clip_rect(clip).rect_stroke(geo.cell_rect(ar, ac), 0.0, Stroke::new(2.0, theme.sel_border), StrokeKind::Inside);
        }
    }

    fn paint_headers(&self, painter: &Painter, geo: &Geo, model: &dyn GridModel, theme: &GridTheme, font: &FontId, small: &FontId) {
        let hrect = Rect::from_min_max(pos2(geo.cells.left(), geo.rect.top()), pos2(geo.cells.right(), geo.cells.top()));
        painter.rect_filled(hrect, 0.0, theme.header_bg);
        let split = geo.cells.left() + geo.frozen_w;
        let sel = self.sel.bounds(geo.rows().max(1), geo.cols());
        let mut panes = Vec::new();
        if let Some((c0, c1)) = geo.visible_cols() {
            panes.push((Rect::from_min_max(pos2(split, hrect.top()), hrect.max), (c0..=c1).collect::<Vec<_>>()));
        }
        if geo.fc > 0 {
            panes.push((Rect::from_min_max(hrect.min, pos2(split, hrect.bottom())), (0..geo.fc).collect()));
        }
        for (clip, cols) in panes {
            let p = painter.with_clip_rect(clip);
            for c in cols {
                let Some(col) = model.columns().get(c as usize) else { continue };
                let rect = Rect::from_min_max(pos2(geo.x(c), hrect.top()), pos2(geo.x_end(c), hrect.bottom()));
                let selected = sel.is_some_and(|(_, c0, _, c1)| (c0..=c1).contains(&c));
                if selected {
                    p.rect_filled(rect, 0.0, theme.header_sel_bg);
                }
                let inner = rect.shrink2(vec2(5.0, 3.0));
                let cp = p.with_clip_rect(inner.intersect(clip));
                let mut x = inner.left();
                if col.primary_key {
                    paint_key(&cp, pos2(x + 5.0, inner.top() + 8.0), theme.pk);
                    x += 13.0;
                }
                let color = if selected { theme.header_sel_text } else { theme.header_text };
                cp.text(pos2(x, inner.top() + 8.0), Align2::LEFT_CENTER, &col.name, font.clone(), color);
                let badge = kind_badge(col.kind);
                let bw = paint_badge(&cp, pos2(inner.left(), inner.bottom() - 6.0), badge, theme);
                cp.text(pos2(inner.left() + bw + 4.0, inner.bottom() - 6.0), Align2::LEFT_CENTER, &col.type_name, small.clone(), theme.header_type_text);
                if let Some((sc, dir)) = self.sort
                    && sc == c as usize
                {
                    let arrow = if dir == SortDir::Asc { "▲" } else { "▼" };
                    cp.text(pos2(inner.right(), inner.top() + 8.0), Align2::RIGHT_CENTER, arrow, small.clone(), theme.header_sel_text);
                }
                p.vline(rect.right() - 0.5, hrect.y_range(), Stroke::new(1.0, theme.header_line));
            }
        }
        painter.hline(geo.rect.x_range(), hrect.bottom() - 0.5, Stroke::new(1.0, theme.header_line));
        // Corner.
        let corner = Rect::from_min_max(geo.rect.min, pos2(geo.cells.left(), geo.cells.top()));
        painter.rect_filled(corner, 0.0, theme.header_bg);
        painter.vline(corner.right() - 0.5, corner.y_range(), Stroke::new(1.0, theme.header_line));
        painter.hline(corner.x_range(), corner.bottom() - 0.5, Stroke::new(1.0, theme.header_line));
    }

    fn paint_gutter(&self, painter: &Painter, geo: &Geo, model: &dyn GridModel, theme: &GridTheme, small: &FontId) {
        let g = Rect::from_min_max(pos2(geo.rect.left(), geo.cells.top()), pos2(geo.cells.left(), geo.cells.bottom()));
        painter.rect_filled(g, 0.0, theme.header_bg);
        let p = painter.with_clip_rect(g);
        if let Some((r0, r1)) = geo.visible_rows() {
            let sel = self.sel.bounds(geo.rows(), geo.cols().max(1));
            for r in r0..=r1 {
                let rect = Rect::from_min_max(pos2(g.left(), geo.y(r)), pos2(g.right(), geo.y_end(r)));
                let selected = sel.is_some_and(|(r0, _, r1, _)| (r0..=r1).contains(&r));
                if selected {
                    p.rect_filled(rect, 0.0, theme.header_sel_bg);
                }
                if r == self.sel.active.0 {
                    // Current-record marker.
                    let c = pos2(rect.left() + 8.0, rect.center().y);
                    p.add(egui::Shape::convex_polygon(vec![pos2(c.x - 3.0, c.y - 4.5), pos2(c.x + 3.5, c.y), pos2(c.x - 3.0, c.y + 4.5)], theme.sel_border, Stroke::NONE));
                }
                let mark = match model.row_state(r as usize) {
                    RowState::Inserted => "*",
                    RowState::Modified => "✎",
                    RowState::Deleted => "×",
                    RowState::Normal => "",
                };
                let label = if mark.is_empty() { (r + 1).to_string() } else { format!("{mark} {}", r + 1) };
                let color = if selected { theme.header_sel_text } else { theme.header_type_text };
                p.text(pos2(rect.right() - 5.0, rect.center().y), Align2::RIGHT_CENTER, label, small.clone(), color);
                p.hline(g.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, theme.gridline));
            }
        }
        painter.vline(g.right() - 0.5, g.y_range(), Stroke::new(1.0, theme.header_line));
    }
}

/// Short marker of a column's kind, drawn before its type name (as database GUIs do).
fn kind_badge(kind: ColumnKind) -> &'static str {
    match kind {
        ColumnKind::Text => "ABC",
        ColumnKind::Number => "#",
        ColumnKind::Bool => "T/F",
        ColumnKind::Date => "DATE",
        ColumnKind::Time => "TIME",
        ColumnKind::DateTime => "DATETIME",
        ColumnKind::Binary => "BIN",
        ColumnKind::Json => "{ }",
    }
}

/// Paints `text` in a small rounded box left-aligned at `pos` (vertically centred); returns its width.
fn paint_badge(p: &Painter, pos: Pos2, text: &str, theme: &GridTheme) -> f32 {
    let font = FontId::monospace(8.0);
    let galley = p.layout_no_wrap(text.to_string(), font, theme.header_type_text);
    let size = galley.size() + vec2(5.0, 1.0);
    let rect = Rect::from_min_size(pos2(pos.x, pos.y - size.y / 2.0), size);
    p.rect_stroke(rect, 2.0, Stroke::new(0.8, theme.header_type_text), StrokeKind::Inside);
    p.galley(rect.min + vec2(2.5, 0.5), galley, theme.header_type_text);
    size.x
}

fn gutter_width(rows: u32) -> f32 {
    let digits = (rows.max(1) as f32).log10().floor() + 1.0;
    (digits.max(2.0) * 7.0 + 30.0).round()
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("");
    let mut out: String = line.chars().take(MAX_CELL_CHARS).collect();
    if out.len() < s.len() && (s.contains('\n') || line.chars().count() > MAX_CELL_CHARS) {
        out.push('…');
    }
    out
}

fn paint_key(p: &Painter, c: Pos2, color: Color32) {
    p.circle_stroke(pos2(c.x - 2.0, c.y), 2.6, Stroke::new(1.4, color));
    p.hline(c.x + 0.5..=c.x + 5.5, c.y, Stroke::new(1.4, color));
    p.vline(c.x + 4.5, c.y..=c.y + 2.5, Stroke::new(1.4, color));
}

/// Vertical scrollbar thumb and track.
fn vthumb(geo: &Geo) -> (Rect, Rect) {
    let track = Rect::from_min_max(pos2(geo.cells.right(), geo.cells.top()), pos2(geo.rect.right(), geo.cells.bottom()));
    let total = geo.ri.total() as f32;
    let view = geo.cells.height();
    let h = if total <= view { track.height() } else { (track.height() * view / total).max(24.0).min(track.height()) };
    let max = geo.max_scroll().y;
    let t = if max > 0.0 { (geo.scroll.y / max).clamp(0.0, 1.0) } else { 0.0 };
    let top = track.top() + t * (track.height() - h);
    (Rect::from_min_max(pos2(track.left() + 2.0, top), pos2(track.right() - 2.0, top + h)), track)
}

fn hthumb(geo: &Geo) -> (Rect, Rect) {
    let track = Rect::from_min_max(pos2(geo.cells.left(), geo.cells.bottom()), pos2(geo.cells.right(), geo.rect.bottom()));
    let total = (geo.ci.total() - geo.ci.start(geo.fc)) as f32;
    let view = geo.cells.width() - geo.frozen_w;
    let w = if total <= view { track.width() } else { (track.width() * view / total.max(1.0)).max(24.0).min(track.width()) };
    let max = geo.max_scroll().x;
    let t = if max > 0.0 { (geo.scroll.x / max).clamp(0.0, 1.0) } else { 0.0 };
    let left = track.left() + t * (track.width() - w);
    (Rect::from_min_max(pos2(left, track.top() + 2.0), pos2(left + w, track.bottom() - 2.0)), track)
}

fn paint_scrollbars(painter: &Painter, geo: &Geo, theme: &GridTheme) {
    let (vt, vtrack) = vthumb(geo);
    let (ht, htrack) = hthumb(geo);
    painter.rect_filled(vtrack, 0.0, theme.scroll_track);
    painter.rect_filled(htrack, 0.0, theme.scroll_track);
    painter.rect_filled(Rect::from_min_max(pos2(geo.cells.right(), geo.cells.bottom()), geo.rect.max), 0.0, theme.scroll_track);
    if geo.max_scroll().y > 0.0 {
        painter.rect_filled(vt, 4.0, theme.scroll_thumb);
    }
    if geo.max_scroll().x > 0.0 {
        painter.rect_filled(ht, 4.0, theme.scroll_thumb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_bounds_rows_and_cols() {
        let s = Selection { anchor: (5, 1), active: (2, 3), kind: SelKind::Cells };
        assert_eq!(s.bounds(10, 10), Some((2, 1, 5, 3)));
        let s = Selection { kind: SelKind::Rows, ..s };
        assert_eq!(s.bounds(10, 4), Some((2, 0, 5, 3)));
        let s = Selection { kind: SelKind::Cols, ..s };
        assert_eq!(s.bounds(10, 4), Some((0, 1, 9, 3)));
        assert_eq!(s.bounds(0, 4), None);
    }

    #[test]
    fn first_line_cuts() {
        assert_eq!(first_line("a\nb"), "a…");
        assert_eq!(first_line("abc"), "abc");
    }
}
