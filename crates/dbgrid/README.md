# dbgrid

A virtualised data grid widget for [egui](https://github.com/emilk/egui), built for SQL result
sets. Part of [DbCraft](../../README.md).

- Paints only the visible cells, so it stays smooth on result sets with millions of rows.
- Column headers show the name, data type and a primary-key marker. A row-number gutter marks
  the current record.
- NULLs are shown in grey. Modified, inserted and deleted rows are highlighted.
- Selection of cells, rows and columns; keyboard navigation; copy as TSV.
- Column resize, with double-click to auto-fit. Frozen leading columns.
- In-cell editor and a context menu (set NULL, add or delete records, sort, filter by value).
- Light and dark palettes.

## Usage

The grid reads data through `GridModel` and never changes it. User actions come back as
`GridEvent`s:

```rust
use dbgrid::{DataGrid, GridEvent, VecModel, GridColumn, ColumnKind};

let model = VecModel {
    columns: vec![
        GridColumn { name: "id".into(), type_name: "int".into(), kind: ColumnKind::Number, primary_key: true, width: None },
        GridColumn { name: "name".into(), type_name: "text".into(), ..Default::default() },
    ],
    rows: vec![vec![Some("1".into()), Some("Ada".into())], vec![Some("2".into()), None]],
};
let mut grid = DataGrid::new("people");

// in your egui update:
for ev in grid.show(ui, &model) {
    match ev {
        GridEvent::CellEdited { row, col, value } => { /* store a pending edit */ }
        GridEvent::SortRequested { col, dir } => { /* re-query with ORDER BY */ }
        _ => {}
    }
}
```

Implement `GridModel` (`row_count`, `columns`, `cell`, and optionally `row_state` and
`editable`) to plug in your own data source.

## Credits

The line index, grid geometry with frozen panes, palette layout and column-resize drag model are
derived from [GridCraft](https://github.com/storytold/gridcraft)'s worksheet grid
(Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0). See
[NOTICE.md](NOTICE.md) and [LICENSE-GRIDCRAFT-MIT](LICENSE-GRIDCRAFT-MIT).

License: MIT OR Apache-2.0.
