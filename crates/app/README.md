# dbcraft (app)

The DbCraft desktop application, built with [eframe](https://github.com/emilk/egui). See the
[main README](../../README.md) for features, usage and command-line options.

| Module | Responsibility |
|---|---|
| `app.rs` | Main window: toolbar, connection tree, tab strip, status bar, notifications |
| `backend.rs` | tokio runtime, cache of open drivers, results channel back to the UI |
| `tabs/objects.rs` | "Oggetti" tab: the objects of the selected database or schema |
| `tabs/table.rs` | Table viewer and editor: paging, filtering, sorting, pending edits |
| `tabs/query.rs` | SQL editor and result tabs |
| `tabs/designer.rs` | Table designer and SQL preview |
| `dialogs.rs` | Connection, export, import and confirmation dialogs |
| `export.rs` | CSV, JSON, SQL and XLSX writers; CSV reader |
| `models.rs` | `GridModel` adapters over result sets and editable pages |
| `store.rs` | Saved connections (JSON) and passwords (OS keychain) |
| `cli.rs` | Command-line options and automated screenshots |

License: MIT OR Apache-2.0.
