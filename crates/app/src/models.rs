//! `GridModel` adapters over result sets and editable table pages.

use std::collections::{HashMap, HashSet};

use db_core::{ResultSet, TableInfo, Value};
use dbgrid::{CellView, ColumnKind, GridColumn, GridModel, RowState};

pub fn kind_of(type_name: &str) -> ColumnKind {
    let t = type_name.to_ascii_lowercase();
    if ["int", "float", "double", "decimal", "numeric", "real", "money", "serial", "number", "year"].iter().any(|k| t.contains(k)) {
        ColumnKind::Number
    } else if t.contains("bool") || t == "bit" {
        ColumnKind::Bool
    } else if ["datetime", "timestamp", "smalldatetime"].iter().any(|k| t.contains(k)) {
        ColumnKind::DateTime
    } else if t.contains("date") {
        ColumnKind::Date
    } else if t.starts_with("time") {
        ColumnKind::Time
    } else if ["blob", "binary", "bytea", "image"].iter().any(|k| t.contains(k)) {
        ColumnKind::Binary
    } else if t.contains("json") {
        ColumnKind::Json
    } else {
        ColumnKind::Text
    }
}

pub fn grid_columns(rs: &ResultSet, info: Option<&TableInfo>) -> Vec<GridColumn> {
    let from_info = |name: &str| info.and_then(|i| i.columns.iter().find(|c| c.name == name));
    if rs.columns.is_empty()
        && let Some(info) = info {
            return info
                .columns
                .iter()
                .map(|c| GridColumn { name: c.name.clone(), type_name: c.data_type.clone(), kind: kind_of(&c.data_type), primary_key: c.primary_key, width: None })
                .collect();
        }
    rs.columns
        .iter()
        .enumerate()
        .map(|(idx, c)| {
            let ci = from_info(&c.name);
            let type_name = ci.map_or_else(|| c.type_name.to_lowercase(), |i| i.data_type.clone());
            let type_name = if type_name == "null" { String::new() } else { type_name };
            let mut kind = kind_of(&type_name);
            let numeric_values = rs.rows.iter().filter_map(|r| r.get(idx)).find(|v| !v.is_null()).is_some_and(Value::is_numeric);
            if kind == ColumnKind::Text && numeric_values {
                kind = ColumnKind::Number;
            }
            GridColumn { name: c.name.clone(), kind, primary_key: ci.is_some_and(|i| i.primary_key), type_name, width: None }
        })
        .collect()
}

fn view(v: &Value) -> CellView<'_> {
    match v {
        Value::Null => CellView::null(),
        Value::Text(t) => CellView::text(t.as_str()),
        other => CellView::text(other.to_string()),
    }
}

/// Read-only view over a query result.
pub struct ResultModel<'a> {
    pub rs: &'a ResultSet,
    pub columns: &'a [GridColumn],
}

impl GridModel for ResultModel<'_> {
    fn row_count(&self) -> usize {
        self.rs.rows.len()
    }
    fn columns(&self) -> &[GridColumn] {
        self.columns
    }
    fn cell(&self, row: usize, col: usize) -> CellView<'_> {
        self.rs.rows.get(row).and_then(|r| r.get(col)).map_or_else(CellView::null, view)
    }
}

/// Pending edits on a browsed table page.
#[derive(Default, Clone)]
pub struct Edits {
    /// (row, col) → new value, for rows of the loaded page.
    pub cells: HashMap<(usize, usize), Value>,
    /// New rows appended after the page; `None` = not set (database default).
    pub inserted: Vec<Vec<Option<Value>>>,
    pub deleted: HashSet<usize>,
}

impl Edits {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.inserted.is_empty() && self.deleted.is_empty()
    }
    pub fn count(&self) -> usize {
        let rows: HashSet<usize> = self.cells.keys().map(|k| k.0).collect();
        rows.len() + self.inserted.len() + self.deleted.len()
    }
}

pub struct TableModel<'a> {
    pub rs: &'a ResultSet,
    /// The visible columns, in display order.
    pub columns: &'a [GridColumn],
    /// Grid column → result-set column.
    pub col_map: &'a [usize],
    pub edits: &'a Edits,
    pub editable: bool,
}

impl TableModel<'_> {
    /// The current value of a cell (pending edit, or original), by result-set column.
    pub fn value(&self, row: usize, data_col: usize) -> Option<Value> {
        let n = self.rs.rows.len();
        if row >= n {
            return self.edits.inserted.get(row - n).and_then(|r| r.get(data_col)).cloned().flatten();
        }
        if let Some(v) = self.edits.cells.get(&(row, data_col)) {
            return Some(v.clone());
        }
        self.rs.rows.get(row).and_then(|r| r.get(data_col)).cloned()
    }
}

impl GridModel for TableModel<'_> {
    fn row_count(&self) -> usize {
        self.rs.rows.len() + self.edits.inserted.len()
    }
    fn columns(&self) -> &[GridColumn] {
        self.columns
    }
    fn cell(&self, row: usize, col: usize) -> CellView<'_> {
        let Some(&dc) = self.col_map.get(col) else { return CellView::null() };
        let n = self.rs.rows.len();
        if row >= n {
            return match self.edits.inserted.get(row - n).and_then(|r| r.get(dc)) {
                Some(Some(v)) => CellView { dirty: true, ..view(v) },
                _ => CellView { text: "(Default)".into(), null: true, dirty: false },
            };
        }
        if let Some(v) = self.edits.cells.get(&(row, dc)) {
            return CellView { dirty: true, ..view(v) };
        }
        self.rs.rows.get(row).and_then(|r| r.get(dc)).map_or_else(CellView::null, view)
    }
    fn row_state(&self, row: usize) -> RowState {
        if row >= self.rs.rows.len() {
            RowState::Inserted
        } else if self.edits.deleted.contains(&row) {
            RowState::Deleted
        } else if self.edits.cells.keys().any(|k| k.0 == row) {
            RowState::Modified
        } else {
            RowState::Normal
        }
    }
    fn editable(&self) -> bool {
        self.editable
    }
}
