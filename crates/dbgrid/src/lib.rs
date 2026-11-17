//! `dbgrid`: a virtualised data grid for egui, tailored to SQL result sets.
//!
//! The geometry, line index, palette layout and drag model are derived from the
//! GridCraft worksheet grid (<https://github.com/storytold/gridcraft>, MIT OR Apache-2.0);
//! see `NOTICE.md`.

pub mod datetime;
mod geo;
mod grid;
pub mod line_index;
mod model;
pub mod theme;

pub use grid::{DataGrid, GridEvent, Selection, SortDir};
pub use model::{CellView, ColumnKind, GridColumn, GridModel, RowState, VecModel};
pub use theme::GridTheme;
