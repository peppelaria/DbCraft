//! The data surface the grid reads: a generic, read-only view over a result set.

use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColumnKind {
    #[default]
    Text,
    Number,
    Bool,
    DateTime,
    Binary,
    Json,
}

#[derive(Clone, Debug, Default)]
pub struct GridColumn {
    pub name: String,
    /// Native type name shown under the column name (e.g. `varchar(255)`).
    pub type_name: String,
    pub kind: ColumnKind,
    pub primary_key: bool,
    /// Initial width in points (`None`: computed from the name).
    pub width: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RowState {
    #[default]
    Normal,
    Modified,
    Inserted,
    Deleted,
}

#[derive(Clone, Debug, Default)]
pub struct CellView<'a> {
    pub text: Cow<'a, str>,
    pub null: bool,
    /// The cell has a pending (unsaved) change.
    pub dirty: bool,
}

impl<'a> CellView<'a> {
    pub fn null() -> Self {
        CellView { text: Cow::Borrowed("(NULL)"), null: true, dirty: false }
    }
    pub fn text(t: impl Into<Cow<'a, str>>) -> Self {
        CellView { text: t.into(), null: false, dirty: false }
    }
}

pub trait GridModel {
    fn row_count(&self) -> usize;
    fn columns(&self) -> &[GridColumn];
    fn cell(&self, row: usize, col: usize) -> CellView<'_>;
    fn row_state(&self, _row: usize) -> RowState {
        RowState::Normal
    }
    fn editable(&self) -> bool {
        false
    }
}

/// A simple in-memory model, handy for demos and tests.
#[derive(Clone, Debug, Default)]
pub struct VecModel {
    pub columns: Vec<GridColumn>,
    pub rows: Vec<Vec<Option<String>>>,
}

impl GridModel for VecModel {
    fn row_count(&self) -> usize {
        self.rows.len()
    }
    fn columns(&self) -> &[GridColumn] {
        &self.columns
    }
    fn cell(&self, row: usize, col: usize) -> CellView<'_> {
        match self.rows.get(row).and_then(|r| r.get(col)) {
            Some(Some(s)) => CellView::text(s.as_str()),
            _ => CellView::null(),
        }
    }
}
