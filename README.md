# DbCraft

**A fast, native, open-source desktop database client written in Rust.**

DbCraft brings the familiar "one window for all your databases" workflow of professional
database GUIs to a lightweight native app: saved connections, an object tree, a spreadsheet-like
data grid that stays smooth on large tables, a SQL editor, a visual table designer, and
import/export, for **MySQL / MariaDB, PostgreSQL, SQL Server and SQLite**.

🇮🇹 [Leggi in italiano](README.it.md)

![Table viewer](docs/screenshots/table-viewer.png)

> **Status: early development (v0.1).** All four engines pass the same end-to-end test suite
> against real servers: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 and SQLite.
> Back up your data before you edit production databases.

---

## Features

| Area | What you get |
|---|---|
| **Connections** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Saved connections, connection test, optional SSL/TLS. Passwords are kept in the OS keychain (macOS Keychain, Windows Credential Manager, Secret Service on Linux), never in plain text. |
| **Object tree** | Connection → Database → Schema → Tables / Views / Functions / Procedures / Triggers, loaded lazily, with context menus. |
| **Objects view** | A searchable list of the objects in the selected database or schema, with row counts and comments. |
| **Table viewer / editor** | Virtualised grid (only visible cells are painted), server-side paging (100 – 50,000 rows per page), `WHERE` filter, server-side sorting, "filter by this value". Edit cells in place, set NULL or an empty string, add and delete records. Changes stay pending (highlighted) until you **Apply**; they are then committed in a single transaction, or **Discard** them. |
| **Data grid** | Column headers with type and primary-key markers, row-number gutter with a current-record marker, NULLs in grey, column resize (double-click to auto-fit), frozen columns, multi-cell selection, keyboard navigation, copy as TSV (with or without headers). |
| **SQL editor** | Syntax highlighting, *Run* (⌘/Ctrl + Enter) and *Run selection*, multiple statements with one result tab each, a message log with affected rows and timing, open and save `.sql` files. |
| **Table designer** | Fields (name, type, not-null, key, auto-increment, default, comment), indexes and foreign keys, with a live **SQL preview** of the `CREATE TABLE` / `ALTER TABLE` statements for the connection's dialect. On SQLite, changes it can't express with `ALTER` are applied by rebuilding the table, keeping its data. |
| **Export** | Tables, views or query results to CSV, JSON, SQL `INSERT` script or Excel (`.xlsx`). |
| **Import** | CSV with a configurable delimiter and header row, a field-mapping preview, and optional empty-cell → NULL. Rows are inserted in batches inside transactions. |
| **UI** | Light and dark themes. The interface is currently in Italian. |

| Objects | SQL editor (dark theme) |
|---|---|
| ![Objects](docs/screenshots/objects.png) | ![Query editor](docs/screenshots/query-editor-dark.png) |

## Getting started

### Requirements

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS, Windows or Linux. Linux needs the usual development packages for a desktop window
  (X11/Wayland, `libxkbcommon`) and D-Bus for the keychain.

### Build and run

```sh
git clone https://github.com/rmpaolillo/DbCraft.git
cd DbCraft
cargo run --release                     # start the app
cargo run --release -- path/to/file.db  # open a SQLite file directly (temporary connection)
cargo run --release -- "postgres://user:password@localhost:5432/mydb"  # or any server by URL
```

URL schemes: `postgres://`, `mysql://`, `mariadb://`, `sqlserver://` (or `mssql://`) and
`sqlite://`. Percent-encode special characters in the password (for example `@` → `%40`).

To create a connection, click **Connessione** in the toolbar and pick the database type.

### Command-line options

| Option | Description |
|---|---|
| `<file.db>` or `<url>` | Open a SQLite file or a server URL as a temporary, unsaved connection |
| `--open-table <name>` | Open a table of that file once it is loaded |
| `--query "<sql>"` | Open a query tab with this SQL and run it |
| `--dark` | Start with the dark theme |
| `--screenshot <out.png> [--delay <s>]` | Save a PNG of the window after `s` seconds (default 3), then quit. Used for automated UI checks. |

Saved connections live in the OS config directory (`dbcraft/connections.json`), for example
`~/Library/Application Support/dbcraft/` on macOS. Set `DBCRAFT_CONFIG_DIR` to use another
folder.

### Tests

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Driver integration tests against real servers (Docker):

```sh
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers -- --test-threads=1
docker compose -f docker-compose.test.yml down -v
```

The suite creates tables with indexes and foreign keys, runs multi-statement scripts, checks
introspection, applies and rolls back transactional edits, alters tables through the designer's
SQL, and decodes each engine's data types (timestamps with time zones, UUID, JSON, arrays,
money, unsigned 64-bit, XML, …). The SQL Server image is amd64-only and runs under emulation on
Apple Silicon.

## Architecture

DbCraft is a Cargo workspace with four crates:

```
crates/
├── dbgrid       Virtualised egui data grid (GridModel trait → DataGrid widget → GridEvent)
├── db-core      Value / ResultSet / schema types, the async Driver trait, SQL dialects and DDL generation
├── db-drivers   SQLite, PostgreSQL, MySQL/MariaDB (sqlx) and SQL Server (tiberius)
└── app          The `dbcraft` desktop application (eframe)
```

- The UI never blocks: database work runs on a background **tokio** runtime, and results come
  back over a channel and trigger a repaint.
- The grid knows nothing about databases. It reads through the small `GridModel` trait and
  reports user intent (`CellEdited`, `SortRequested`, `DeleteRows`, …) as events, so the app
  decides what to do.
- SQL that differs between engines (identifier quoting, literals, paging, `INSERT`, `UPDATE`,
  `DELETE`, `CREATE`, `ALTER`) lives in one place, `db_core::Dialect`, and is unit-tested.

## Roadmap

- [x] Integration tests against PostgreSQL, MySQL/MariaDB and SQL Server (Docker)
- [ ] SQL autocompletion (keywords, tables, columns)
- [ ] Cancel a running query
- [ ] SSH tunnel and more SSL options
- [ ] Custom vector icon set
- [ ] English UI / localisation
- [ ] ER diagram, data and structure sync, backup/restore
- [ ] More engines (Oracle, MongoDB, Redis)

Issues and pull requests are welcome.

## Credits

DbCraft is built on the work of many open-source projects. Thank you!

- **[GridCraft](https://github.com/storytold/gridcraft)**: the data grid in `crates/dbgrid` is
  derived from GridCraft's worksheet grid (the prefix-sum line index, grid geometry with frozen
  panes, theme palette layout and column-resize drag model), adapted to SQL result sets.
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0. See
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** by Emil Ernerfeldt and contributors: the
  immediate-mode GUI toolkit.
- **[sqlx](https://github.com/launchbadge/sqlx)**: SQLite, PostgreSQL and MySQL drivers.
- **[tiberius](https://github.com/prisma/tiberius)**: SQL Server (TDS) driver.
- **[tokio](https://tokio.rs)**: async runtime.
- **[SQLite](https://sqlite.org)** (public domain), bundled through `libsqlite3-sys`.
- **[syntect](https://github.com/trishume/syntect)** (via `egui_extras`): SQL syntax highlighting.
- **[rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter)**,
  **[csv](https://github.com/BurntSushi/rust-csv)**, **[serde](https://serde.rs)**: export and import.
- **[keyring](https://github.com/hwchen/keyring-rs)**, **[rfd](https://github.com/PolyMeilex/rfd)**,
  **[dirs](https://github.com/soc/dirs-rs)**: OS keychain, native file dialogs and config paths.

The full list of third-party components and their licences is in [CREDITS.md](CREDITS.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in
this work, as defined in the Apache-2.0 license, is dual licensed as above, without any
additional terms or conditions.

## Disclaimer

DbCraft is an independent project. It is not affiliated with, endorsed by or sponsored by
PremiumSoft CyberTech Ltd. "Navicat" is a trademark of its owner. It is mentioned only to
describe the kind of tool DbCraft is, and DbCraft uses none of its code, artwork or assets.
MySQL, MariaDB, PostgreSQL, SQL Server and SQLite are trademarks of their owners.
