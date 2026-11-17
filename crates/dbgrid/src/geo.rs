//! Geometry of the grid for one frame: headers, the row-number gutter, frozen columns and
//! scrolling. Derived from GridCraft `grid::geo::Geo` (MIT OR Apache-2.0), see NOTICE.md;
//! zoom, outlines, right-to-left and page layout were dropped.

use std::sync::Arc;

use egui::{Pos2, Rect, pos2};

use crate::line_index::LineIndex;

#[derive(Clone)]
pub struct Geo {
    pub rect: Rect,
    pub cells: Rect,
    /// Frozen leading columns.
    pub fc: u32,
    pub frozen_w: f32,
    pub scroll: egui::Vec2,
    pub ri: Arc<LineIndex>,
    pub ci: Arc<LineIndex>,
}

impl Geo {
    pub fn new(rect: Rect, cells: Rect, fc: u32, scroll: egui::Vec2, ri: Arc<LineIndex>, ci: Arc<LineIndex>) -> Geo {
        let fc = fc.min(ci.len());
        let frozen_w = ci.start(fc) as f32;
        Geo { rect, cells, fc, frozen_w, scroll, ri, ci }
    }

    pub fn rows(&self) -> u32 {
        self.ri.len()
    }
    pub fn cols(&self) -> u32 {
        self.ci.len()
    }

    pub fn x(&self, col: u32) -> f32 {
        if col < self.fc {
            self.cells.left() + self.ci.start(col) as f32
        } else {
            self.cells.left() + self.frozen_w + (self.ci.start(col) as f32 - self.ci.start(self.fc) as f32 - self.scroll.x)
        }
    }
    pub fn y(&self, row: u32) -> f32 {
        self.cells.top() + (self.ri.start(row) as f32 - self.scroll.y)
    }
    pub fn x_end(&self, col: u32) -> f32 {
        self.x(col) + self.ci.size(col) as f32
    }
    pub fn y_end(&self, row: u32) -> f32 {
        self.y(row) + self.ri.size(row) as f32
    }
    pub fn cell_rect(&self, row: u32, col: u32) -> Rect {
        Rect::from_min_max(pos2(self.x(col), self.y(row)), pos2(self.x_end(col), self.y_end(row)))
    }
    pub fn col_at(&self, x: f32) -> u32 {
        let dx = x - self.cells.left();
        if dx < self.frozen_w {
            self.ci.at(f64::from(dx.max(0.0)))
        } else {
            self.ci.at(self.ci.start(self.fc) + f64::from(self.scroll.x) + f64::from(dx - self.frozen_w))
        }
    }
    pub fn row_at(&self, y: f32) -> u32 {
        self.ri.at(f64::from(self.scroll.y) + f64::from(y - self.cells.top()))
    }
    pub fn in_cells(&self, p: Pos2) -> bool {
        self.cells.contains(p) && p.x < self.cells.left() + self.content_w() && p.y < self.cells.top() + self.content_h()
    }
    /// Visible scrolled rows (inclusive). `None` when there are no rows.
    pub fn visible_rows(&self) -> Option<(u32, u32)> {
        if self.ri.is_empty() {
            return None;
        }
        let r0 = self.ri.at(f64::from(self.scroll.y));
        let r1 = self.ri.at(f64::from(self.scroll.y + self.cells.height()));
        Some((r0, r1))
    }
    /// Visible scrolled (non-frozen) columns (inclusive).
    pub fn visible_cols(&self) -> Option<(u32, u32)> {
        if self.fc >= self.ci.len() {
            return None;
        }
        let left = self.ci.start(self.fc) + f64::from(self.scroll.x);
        let c0 = self.ci.at(left).max(self.fc);
        let c1 = self.ci.at(left + f64::from(self.cells.width() - self.frozen_w));
        Some((c0, c1))
    }
    /// Scrollable extent beyond the viewport.
    pub fn max_scroll(&self) -> egui::Vec2 {
        let w = (self.ci.total() - self.ci.start(self.fc)) as f32;
        let h = self.ri.total() as f32;
        egui::vec2((w - (self.cells.width() - self.frozen_w)).max(0.0), (h - self.cells.height() + 4.0).max(0.0))
    }
    fn content_w(&self) -> f32 {
        self.frozen_w + (self.ci.total() - self.ci.start(self.fc)) as f32 - self.scroll.x
    }
    fn content_h(&self) -> f32 {
        self.ri.total() as f32 - self.scroll.y
    }
}
