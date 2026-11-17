# Notice / Provenienza

Parts of this crate are derived from **GridCraft** (<https://github.com/storytold/gridcraft>),
commit `d60709a1738d4f02c7f507848441306c0aacf692`, licensed under `MIT OR Apache-2.0`,
Copyright (c) 2026 ArtCraft Team and the GridCraft contributors. See `LICENSE-GRIDCRAFT-MIT`.

| File | Derived from |
|---|---|
| `src/line_index.rs` | `crates/model/src/sheet.rs` (`LineIndex`) |
| `src/geo.rs` | `crates/ui-egui/src/grid/geo.rs` |
| `src/theme.rs` | token layout of `crates/ui-egui/src/theme.rs` |
| `src/grid.rs` | painting order and drag model of `crates/ui-egui/src/grid/*` |

Changes: removed the coupling to GridCraft's `Workbook` / `Sheet` / `Session` and replaced it
with the generic `GridModel` trait for SQL result sets. Zoom, outlines, right-to-left layout,
page layout, formulas and conditional formatting were dropped.

---

Parti di questo crate derivano da **GridCraft** (commit e licenza indicati sopra). Il codice è
stato adattato: l'accoppiamento con `Workbook` / `Sheet` / `Session` è stato sostituito da un
trait generico `GridModel` pensato per i result set SQL.
