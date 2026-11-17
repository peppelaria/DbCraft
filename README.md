# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

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
| **Connections** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Saved connections, connection test, **SSL/TLS** (server verification against a CA and host name, client certificate authentication), an **SSH tunnel** with password or private-key authentication (with or without a passphrase; the host key is checked against `~/.ssh/known_hosts`), and an **HTTP tunnel** through DbCraft's PHP script for databases reachable only from a web server ([tunnel/README.md](tunnel/README.md)). Passwords are kept in the OS keychain (macOS Keychain, Windows Credential Manager, Secret Service on Linux), never in plain text. |
| **Object tree** | Connection → Database → Schema → Tables / Views / Functions / Procedures / Triggers, loaded lazily, with context menus. |
| **Objects view** | A searchable list of the objects in the selected database or schema, with row counts and comments. |
| **Table viewer / editor** | Virtualised grid (only visible cells are painted), server-side paging (100 – 50,000 rows per page). **Filter & Sort** panel: conditions read as sentences of clickable tokens (field · operator · value), each linked to the next by its own *and*/*or*, with nested bracketed groups and 21 operators (=, !=, <…, contains, begins/ends with and their negations, is NULL/empty, between, in list, custom SQL), or a raw `WHERE`; plus multi-column sorting. Show/hide columns. **Grid, Form and Text views**. **Cell editor** side panel for long text, JSON (pretty-print) and binary (hex dump). **Date/time picker** on date, time and datetime/timestamp columns (calendar button on the active cell or Alt+↓, also in the form view and cell editor), keeping fractional seconds and time zones. Edit in place, set NULL, add and delete records (deletes ask for confirmation). Auto-commit saves a record when you leave it; **Begin Transaction** holds every change until **Commit** or **Rollback**. |
| **Data grid** | Column headers with type and primary-key markers, row-number gutter with a current-record marker, NULLs in grey, column resize (double-click to auto-fit), frozen columns, multi-cell selection, keyboard navigation, copy as TSV (with or without headers). |
| **SQL editor** | Syntax highlighting, *Run* (⌘/Ctrl + Enter) and *Run selection*, multiple statements with one result tab each, a message log with affected rows and timing, open and save `.sql` files. |
| **Query Builder** | Visual query design: drag tables onto a canvas, tick columns, drag a column onto another table's column to join (INNER / LEFT / RIGHT / FULL, shown as curves); foreign keys join automatically. Aggregates with automatic GROUP BY, aliases, conditions and sorting with the same token editor as the table filter, DISTINCT and LIMIT (TOP on SQL Server), live SQL preview; **OK** puts the SQL in the editor. |
| **Table designer** | Fields (name, type, not-null, key, auto-increment, default, comment), indexes and foreign keys, with a live **SQL preview** of the `CREATE TABLE` / `ALTER TABLE` statements for the connection's dialect. On SQLite, changes it can't express with `ALTER` are applied by rebuilding the table, keeping its data. |
| **Export** | Tables, views or query results to CSV, JSON, SQL `INSERT` script or Excel (`.xlsx`). |
| **Import** | CSV with a configurable delimiter and header row, a field-mapping preview, and optional empty-cell → NULL. Rows are inserted in batches inside transactions. |
| **UI** | Light and dark themes. The interface is currently in Italian. |

| Objects | SQL editor (dark theme) |
|---|---|
| ![Objects](docs/screenshots/objects.png) | ![Query editor](docs/screenshots/query-editor-dark.png) |

## Getting started

### Download

Prebuilt packages are on the [Releases](https://github.com/rmpaolillo/DbCraft/releases) page:
a universal `.app` for macOS 11+, Windows x64, and Linux x64/arm64 (glibc 2.35+). The builds are
not notarised: on macOS, open the app with right-click → Open the first time (or run
`xattr -dr com.apple.quarantine /Applications/DbCraft.app`); on Windows, SmartScreen may ask for
confirmation.

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
Add `?ssh=user:password@ssh-host:port` to go through an SSH tunnel, or
`?ssh=user@ssh-host&ssh_key=~/.ssh/id_ed25519` for key authentication.

To create a connection, click **Connessione** in the toolbar and pick the database type.

### Command-line options

| Option | Description |
|---|---|
| `<file.db>` or `<url>` | Open a SQLite file or a server URL as a temporary, unsaved connection |
| `--open-table <name>` | Open a table of that file once it is loaded |
| `--query "<sql>"` | Open a query tab with this SQL and run it |
| `--dark` | Start with the dark theme |
| `--new-connection <kind> [--ssh-tab]` | Open the new-connection dialog (`mysql`, `postgres`, …), optionally on its SSH tab |
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
scripts/test-certs.sh && mkdir -p target/ssh-test   # test certificates, SSH key folder
docker compose -f docker-compose.test.yml up -d --wait
DBCRAFT_TEST_SERVERS=1 cargo test -p db-drivers --test servers --test ssh --test tls --test http -- --test-threads=1
docker compose -f docker-compose.test.yml down -v
```

The suite creates tables with indexes and foreign keys, runs multi-statement scripts, checks
introspection, applies and rolls back transactional edits, alters tables through the designer's
SQL, and decodes each engine's data types (timestamps with time zones, UUID, JSON, arrays,
money, unsigned 64-bit, XML, …). The SSH tests go through an OpenSSH server container with
password, RSA and passphrase-protected Ed25519 keys; the SSL tests use PostgreSQL and MySQL
with test certificates (verify-full, client certificates, a MySQL `REQUIRE X509` user). The SQL Server image is amd64-only and runs under emulation on
Apple Silicon.

### Releasing (maintainers)

Releases are built and published by GitHub Actions **automatically**:

1. Bump `version` in `[workspace.package]` in `Cargo.toml` (e.g. `0.1.0` → `0.2.0`), commit and
   push to `main`.
2. The **Release** workflow notices the new version (if no release `v<version>` exists yet), runs
   clippy and the tests, builds macOS (universal), Windows x64, Linux x64 and Linux arm64, then
   creates the tag `v<version>` and the GitHub release with the packages and `SHA256SUMS.txt`.
   Versions below 1.0 or with a suffix (`1.0.0-beta.1`) are marked as pre-releases.

Pushes that do not change the version do not create releases. The workflow can still be started
by hand (**Actions → Release → Run workflow**), e.g. to make a draft release.

Every push and pull request also runs **CI** (`.github/workflows/ci.yml`): clippy, tests on
Linux, macOS and Windows, and the driver suite against real PostgreSQL, MySQL, MariaDB and SQL
Server service containers.

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
- [x] SSH tunnel (password, private key with optional passphrase)
- [x] SSL options (CA verification, host name verification, client certificates)
- [x] HTTP tunnel (DbCraft PHP script, MySQL/MariaDB and PostgreSQL)
- [ ] Custom vector icon set
- [ ] English UI / localisation
- [ ] ER diagram, data and structure sync, backup/restore
- [ ] More engines (Oracle, MongoDB, Redis)

Issues and pull requests are welcome.

## Credits

DbCraft is built on the work of many open-source projects. Thank you!

- **[GridCraft](https://github.com/storytold/gridcraft)**: the code-drawn **icon set** (extended
  with database icons in the same style) and the data grid in `crates/dbgrid` are
  derived from GridCraft (the grid: prefix-sum line index, geometry with frozen panes, palette
  layout and column-resize drag model, adapted to SQL result sets).
  Copyright © 2026 ArtCraft Team and the GridCraft contributors, MIT OR Apache-2.0. See
  [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).
- **[egui / eframe](https://github.com/emilk/egui)** by Emil Ernerfeldt and contributors: the
  immediate-mode GUI toolkit.
- **[sqlx](https://github.com/launchbadge/sqlx)**: SQLite, PostgreSQL and MySQL drivers.
- **[tiberius](https://github.com/prisma/tiberius)**: SQL Server (TDS) driver.
- **[tokio](https://tokio.rs)**: async runtime.
- **[reqwest](https://github.com/seanmonstar/reqwest)**: HTTP client for the HTTP tunnel.
- **[russh](https://github.com/Eugeny/russh)**: SSH client for the tunnel.
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
