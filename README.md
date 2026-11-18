# DbCraft

[![CI](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml/badge.svg)](https://github.com/rmpaolillo/DbCraft/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rmpaolillo/DbCraft?include_prereleases)](https://github.com/rmpaolillo/DbCraft/releases)

**A fast, native, open-source desktop database client written in Rust.**

DbCraft brings the familiar "one window for all your databases" workflow of professional
database GUIs to a lightweight native app: saved connections, an object tree, a spreadsheet-like
data grid that stays smooth on large tables, a SQL editor, a visual table designer, and
import/export, for **MySQL / MariaDB, PostgreSQL, SQL Server and SQLite**.

*English · [Italiano](README.it.md) · [Español](README.es.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Українська](README.uk.md) · [中文](README.zh.md) · [日本語](README.ja.md) · [한국어](README.ko.md)*

![DbCraft: the objects of a MySQL database, with the menu bar, the toolbar and the connection tree](docs/screenshots/objects.png)

> **Status: early development (v0.1).** All four engines pass the same end-to-end test suite
> against real servers: PostgreSQL 17, MySQL 8.4, MariaDB 11.4, SQL Server 2022 and SQLite.
> Back up your data before you edit production databases.

---

## Features

| Area | What you get |
|---|---|
| **Connections** | MySQL, MariaDB, PostgreSQL, SQL Server, SQLite. Saved connections, connection test, **SSL/TLS** (server verification against a CA and host name, client certificate authentication), an **SSH tunnel** with password or private-key authentication (with or without a passphrase; the host key is checked against `~/.ssh/known_hosts`), and an **HTTP tunnel** through DbCraft's PHP script for databases reachable only from a web server ([tunnel/README.md](tunnel/README.md)). Passwords are kept in the OS keychain (macOS Keychain, Windows Credential Manager, Secret Service on Linux), never in plain text. |
| **Object tree** | Connection → Database → Schema → Tables / Views / Functions / Procedures / Triggers, loaded lazily, with context menus. At the bottom: **search** (filters the tree as you type), **★ only favorites** and **collapse all**. |
| **Menu bar** | File, Edit, View, **Favorites**, Tools, Window, Help. Any object can be added to the favorites (right-click › *Add to favorites*); the Favorites menu opens it, connecting first if needed. |
| **Objects view** | A searchable list of the objects in the selected database or schema, as in Navicat: name, auto increment, modified date, data length, engine, rows and comment (the columns the server provides). The toolbar's **Table / View / Function / Others ▾** switch the kind of object: *Others* lists triggers and, depending on the server, indexes, sequences, events, types, extensions and synonyms. |
| **Saved queries** | The toolbar's **Query** lists the queries saved for the database (⌘/Ctrl + S in a query tab saves there): open, new, delete. |
| **Users** | Users and roles of the server with their privileges; **new user**, **change password**, **grant / revoke** privileges on a database or schema, **delete user**. The SQL is shown before it runs (password hidden). |
| **Backup / restore** | A backup is an SQL script with the structure and data of every table (in foreign-key order) and the views; the **Backup** tab lists them, restores one (or any `.sql` file) and deletes them. Tested round trip on all five engines. |
| **Automation** | Scheduled jobs: a backup or some SQL, every *n* minutes, every day or every week at a given time, with next/last run and outcome, and **Run now**. Jobs run while DbCraft is open. |
| **Model** | The ER diagram of a database or schema: a box per table with its columns (primary and foreign keys marked) and a line per foreign key. Drag boxes, pan, zoom, **Arrange**, **Export SQL**; double-click a table to design it. |
| **Table viewer / editor** | Virtualised grid (only visible cells are painted), server-side paging (100 – 50,000 rows per page). **Filter & Sort** panel: conditions read as sentences of clickable tokens (field · operator · value), each linked to the next by its own *and*/*or*, with nested bracketed groups and 21 operators (=, !=, <…, contains, begins/ends with and their negations, is NULL/empty, between, in list, custom SQL), or a raw `WHERE`; plus multi-column sorting. Show/hide columns. **Grid, Form and Text views**. **Cell editor** side panel for long text, JSON (pretty-print) and binary (hex dump). **Date/time picker** on date, time and datetime/timestamp columns (calendar button on the active cell or Alt+↓, also in the form view and cell editor), keeping fractional seconds and time zones. Edit in place, set NULL, add and delete records (deletes ask for confirmation). Auto-commit saves a record when you leave it; **Begin Transaction** holds every change until **Commit** or **Rollback**. |
| **Data grid** | Column headers with type and primary-key markers, row-number gutter with a current-record marker, NULLs in grey, column resize (double-click to auto-fit), frozen columns, multi-cell selection, keyboard navigation, copy as TSV (with or without headers). |
| **SQL editor** | Syntax highlighting; **code completion** (keywords, tables, views, functions and columns, also after an alias `u.`; Ctrl+Space); connection, database and schema pickers in the toolbar; *Run* all (⌘/Ctrl + Enter), the selection or the statement at the cursor (⌘/Ctrl + Shift + Enter); **Stop** (also cancels the query on the server with PostgreSQL and MySQL/MariaDB); **Explain** (execution plan; SHOWPLAN on SQL Server); **Beautify SQL** (⌘/Ctrl + Shift + F); ready-made **snippets**; one result tab per statement, a message log, Ln/Col in the status bar, open and save `.sql` files. |
| **Query Builder** | Visual query design: drag tables onto a canvas, tick columns, drag a column onto another table's column to join (INNER / LEFT / RIGHT / FULL, shown as curves); foreign keys join automatically. Aggregates with automatic GROUP BY, aliases, conditions and sorting with the same token editor as the table filter, DISTINCT and LIMIT (TOP on SQL Server), live SQL preview; **OK** puts the SQL in the editor. |
| **Table designer** | Fields (name, type, not-null, key, auto-increment, default, comment), indexes and foreign keys, with a live **SQL preview** of the `CREATE TABLE` / `ALTER TABLE` statements for the connection's dialect. On SQLite, changes it can't express with `ALTER` are applied by rebuilding the table, keeping its data. |
| **Export** | Tables, views or query results to CSV, JSON, SQL `INSERT` script or Excel (`.xlsx`). |
| **Import** | CSV with a configurable delimiter and header row, a field-mapping preview, and optional empty-cell → NULL. Rows are inserted in batches inside transactions. |
| **AI chat** | An AI assistant in a panel on the right (**Chat AI** in the toolbar), the same as OpenSlate's: it writes, explains, fixes and optimises SQL, and works on your databases with tools. It sees the configured connections, reads structure and data (read-only queries), opens queries in a new editor tab, and makes changes only after your **yes** in the chat. Any OpenAI-compatible service (LM Studio, Ollama, OpenAI, Anthropic Claude, Google Gemini, DeepSeek, OpenRouter…), tried in order with fallback; saved conversations, Markdown and formulas, a context meter with automatic compaction, and **AI settings** (agents, keys in the keychain, test, models, context). |
| **Languages** | The interface speaks **English, Italiano, Español, Français, Deutsch, Українська, 中文, 日本語 and 한국어**: the system language by default, or the one chosen in **View › Language** (it applies at once). Chinese, Japanese and Korean use a font of the system. |
| **UI** | Light and dark themes. |

| Table viewer | SQL editor (dark theme) |
|---|---|
| ![Table viewer with the Filter & Sort panel](docs/screenshots/table-viewer.png) | ![SQL editor with code completion, dark theme](docs/screenshots/query-editor-dark.png) |
| **Model (ER diagram)** | **Users and privileges** |
| ![ER diagram of a database](docs/screenshots/model.png) | ![Users of a MySQL server with their privileges](docs/screenshots/users.png) |
| **Backup** | **Automation** |
| ![Backups of a database](docs/screenshots/backup.png) | ![Scheduled backup and query jobs](docs/screenshots/automation.png) |

## Getting started

### Download

Prebuilt packages are on the [Releases](https://github.com/rmpaolillo/DbCraft/releases) page:

- **macOS 11+ (Apple silicon)**: `DbCraft-<version>-macos-arm64.dmg`, then drag DbCraft to
  Applications. Builds without an Apple Developer ID are signed ad hoc: allow the app once with
  right-click → **Open**, or System Settings → Privacy & Security → **Open Anyway**.
- **Debian / Ubuntu**: `sudo apt install ./dbcraft_<version>_amd64.deb`
- **Other Linux (x86_64, glibc 2.35+)**: `dbcraft-<version>-linux-x86_64.tar.gz`
- **Windows 10/11 (x64)**: `dbcraft-<version>-windows-x86_64.zip` (the Visual C++ runtime is
  included); SmartScreen may ask for confirmation.

#### Updates

An installed release checks GitHub for a newer release at start and every 12 hours. When there
is one, a bar under the toolbar offers **Update now** (download, SHA-256 check against
`SHA256SUMS.txt`, install in place of this copy, then **Restart now**), **What's new** (release
notes) and **Skip this version**. The macOS app is swapped in place (the previous one is kept
in the config folder under `updates/`), the Linux `.tar.gz` folder gets the new files, the
`.deb` is installed with `pkexec dpkg -i`, and on Windows a helper swaps the files when the app
closes. Saved connections, settings and keychain passwords are not touched. The toolbar button
**Updates** checks by hand and turns the notice or the automatic install on and off.
Builds from source do not update themselves (`git pull`); to try the bar anyway, run with
`DBCRAFT_UPDATE_TEST=1 DBCRAFT_PRETEND_VERSION=0.0.1`.

#### AI assistant

**Chat AI** in the toolbar (or Tools › Chat AI) opens the chat on the right; **Tools › AI settings** opens
its settings. Both are the pages of [OpenSlate](https://github.com/rmpaolillo/OpenSlate), with the same
logic: the agents are tried **in order**, and if one does not answer (server off, no key, error) the
next one does. They live in `ai.toml` in the config folder; a key is never written there in clear:
`keychain:NAME` (a key typed in the settings page goes to the OS keychain), `env:NAME` or
`dsh:NAME` (DeepSeek Harness).

The chat knows which database is selected in DbCraft, and it has tools:

| Tool | What it does |
|---|---|
| `db_connections`, `db_databases`, `db_tables`, `db_describe` | the configured connections and the structure of their databases |
| `db_query` | runs **read-only** SQL (SELECT, WITH, SHOW, EXPLAIN…), at most `max_rows` rows |
| `db_open_query` | opens a query in a new editor tab, without running it |
| `db_execute` | changes data or structure: first it only prepares the change and you see the exact SQL, then it runs **only after your yes** in the next message (and only if *AI settings › The database* allows changes) |

Every SQL block in an answer has **Open in editor** and **Copy**. With a local model (LM Studio,
Ollama) nothing leaves your computer; with a cloud service, the questions and the data the AI reads
go to that service.

### Requirements

- [Rust](https://rustup.rs) **1.95+** (edition 2024)
- macOS, Windows or Linux. Linux needs the usual development packages for a desktop window
  (X11/Wayland, `libxkbcommon`), D-Bus for the keychain and WebKitGTK 4.1 for the AI chat
  (`libwebkit2gtk-4.1-dev`).

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

To create a connection, click **Connection** in the toolbar and pick the database type.

### Command-line options

| Option | Description |
|---|---|
| `<file.db>` or `<url>` | Open a SQLite file or a server URL as a temporary, unsaved connection |
| `--open-table <name>` | Open a table of that file once it is loaded |
| `--query "<sql>"` | Open a query tab with this SQL and run it |
| `--dark` | Start with the dark theme |
| `--lang <code>` | Interface language for this run: `en`, `it`, `es`, `fr`, `de`, `uk`, `zh`, `ja`, `ko` |
| `--open <tool>` | Open a tool once the database is loaded: `users`, `backup`, `automation`, `model`, `queries`, `others:<kind>` |
| `--new-connection <kind> [--ssh-tab]` | Open the new-connection dialog (`mysql`, `postgres`, …), optionally on its SSH tab |
| `--screenshot <out.png> [--delay <s>]` | Save a PNG of the window after `s` seconds (default 3), then quit. Used for automated UI checks. |

Saved connections live in the OS config directory (`dbcraft/connections.json`), for example
`~/Library/Application Support/dbcraft/` on macOS, next to `settings.json`, `favorites.json`,
`automation.json` and the `queries/` and `backups/` folders. Set `DBCRAFT_CONFIG_DIR` to use
another folder.

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
DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1   # backup/restore, users and catalogs
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

A release is published only when you decide: **Actions → Release → Run workflow** (branch
`main`). There is nothing to edit first: the version number is ready.

- The version is `MAJOR.MINOR` from `Cargo.toml` (`[workspace.package] version`) plus a `PATCH`
  counted from the existing tags: no `v0.1.*` tag yet → `v0.1.0`; last tag `v0.1.4` → `v0.1.5`.
- To start a new series (`0.2.0`, `1.0.0`…) change the version in `Cargo.toml` and push; the
  next release starts at `.0`.
- The number is written into `Cargo.toml`/`Cargo.lock` only inside the build: nothing is
  committed back to the repository.

The workflow tests and builds macOS Apple silicon (`.dmg`), Linux x86_64 (`.deb` and `.tar.gz`)
and Windows x86_64 (`.zip`), then publishes the release `v<version>` with the packages and
`SHA256SUMS.txt`. Only the two newest releases are kept: older releases and their tags are
removed.

Optional macOS signing: with the repository secrets `APPLE_CERTIFICATE` (base64 `.p12` with a
"Developer ID Application" identity), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`
(app-specific password) and `APPLE_TEAM_ID`, the app and the DMG are signed with the Developer ID
and notarized; without them they are signed ad hoc.

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

- **Translations** work as in OpenSlate: the Italian text in the code is the key, and
  `crates/app/src/i18n/<lang>.json` gives it in the other languages (placeholders such as
  `{n}` or `{}` also translate texts built at run time). Run with
  `DBCRAFT_I18N_MISSING=missing.txt` to collect the texts shown without a translation.

- The UI never blocks: database work runs on a background **tokio** runtime, and results come
  back over a channel and trigger a repaint.
- The grid knows nothing about databases. It reads through the small `GridModel` trait and
  reports user intent (`CellEdited`, `SortRequested`, `DeleteRows`, …) as events, so the app
  decides what to do.
- SQL that differs between engines (identifier quoting, literals, paging, `INSERT`, `UPDATE`,
  `DELETE`, `CREATE`, `ALTER`) lives in one place, `db_core::Dialect`, and is unit-tested.

## Roadmap

- [x] Integration tests against PostgreSQL, MySQL/MariaDB and SQL Server (Docker)
- [x] SQL autocompletion (keywords, tables, columns)
- [x] Cancel a running query
- [x] SSH tunnel (password, private key with optional passphrase)
- [x] SSL options (CA verification, host name verification, client certificates)
- [x] HTTP tunnel (DbCraft PHP script, MySQL/MariaDB and PostgreSQL)
- [x] Custom vector icon set
- [x] Interface in nine languages
- [x] ER diagram, backup/restore, scheduled jobs, users and privileges
- [x] AI chat for queries and databases (as in OpenSlate)
- [ ] Data and structure sync, BI
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
- **[sys-locale](https://github.com/1Password/sys-locale)**: the system language.
- **[Noto Sans CJK](https://github.com/notofonts/noto-cjk)** (SIL OFL 1.1): a tiny subset for the
  language names 中文 日本語 한국어 in the menu, as in OpenSlate.
- **[OpenSlate](https://github.com/rmpaolillo/OpenSlate)**: the translation system, the self-update and the AI
  chat (its pages, the provider chain and the settings).
- **[wry](https://github.com/tauri-apps/wry)**: the WebViews of the AI chat inside the window.
- **[KaTeX](https://katex.org)**, **[marked](https://marked.js.org)**, **[DOMPurify](https://github.com/cure53/DOMPurify)**
  and the **[Inter](https://rsms.me/inter/)** font, bundled for the chat pages (see `crates/app/web/vendor`).

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
