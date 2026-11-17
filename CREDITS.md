# Credits and third-party notices

DbCraft is distributed under `MIT OR Apache-2.0` (see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE)). It includes code derived from, and links against, the
open-source projects below. We are grateful to their authors and contributors.

## Derived code

### GridCraft

- Project: <https://github.com/storytold/gridcraft>
- Commit used: `d60709a1738d4f02c7f507848441306c0aacf692`
- License: MIT OR Apache-2.0
- Copyright © 2026 ArtCraft Team and the GridCraft contributors
- Used in: `crates/dbgrid`
  - `line_index.rs` is derived from `crates/model/src/sheet.rs` (`LineIndex`).
  - `geo.rs` is derived from `crates/ui-egui/src/grid/geo.rs`.
  - `theme.rs` follows the token layout of `crates/ui-egui/src/theme.rs`.
  - `grid.rs` follows the painting order and drag model of `crates/ui-egui/src/grid/*`.

- Used in: `crates/app/src/icons` — the **icon set**:
  - `gridcraft.rs` is GridCraft's code-drawn icon catalogue (`crates/ui-egui/src/icons.rs`),
    adapted to DbCraft (theme tokens replaced, proportional font for glyph icons);
  - `mod.rs` reuses GridCraft's icon palette (category colours and paper, light and dark, from
    `crates/ui-egui/src/theme.rs`);
  - `db.rs` adds database icons (connection, database, schema, view, procedure, trigger, key,
    index, user, backup, automation, model, BI, transaction, commit/rollback, query builder…)
    drawn for DbCraft in the same style, on the same 16×16 grid with the same helpers.

The adaptations remove the coupling to GridCraft's workbook engine and replace it with a generic
`GridModel` trait for SQL result sets. GridCraft's MIT license text is kept in
[`crates/dbgrid/LICENSE-GRIDCRAFT-MIT`](crates/dbgrid/LICENSE-GRIDCRAFT-MIT), and provenance is
recorded in [`crates/dbgrid/NOTICE.md`](crates/dbgrid/NOTICE.md).

## Direct dependencies

| Crate | Purpose | License |
|---|---|---|
| [egui](https://github.com/emilk/egui), [eframe](https://github.com/emilk/egui), [egui_extras](https://github.com/emilk/egui) | GUI toolkit, native window, tables and syntax highlighting | MIT OR Apache-2.0 |
| [sqlx](https://github.com/launchbadge/sqlx) | SQLite, PostgreSQL and MySQL/MariaDB drivers | MIT OR Apache-2.0 |
| [tiberius](https://github.com/prisma/tiberius) | SQL Server (TDS) driver | MIT OR Apache-2.0 |
| [russh](https://github.com/Eugeny/russh) | SSH client (tunnel) | Apache-2.0 |
| [reqwest](https://github.com/seanmonstar/reqwest) | HTTP client (HTTP tunnel) | MIT OR Apache-2.0 |
| [rustls](https://github.com/rustls/rustls) | TLS; DbCraft selects its `ring` crypto provider | Apache-2.0 OR ISC OR MIT |
| [tokio](https://github.com/tokio-rs/tokio), [tokio-util](https://github.com/tokio-rs/tokio) | Async runtime | MIT |
| [futures-util](https://github.com/rust-lang/futures-rs) | Stream utilities | MIT OR Apache-2.0 |
| [async-trait](https://github.com/dtolnay/async-trait) | Async trait methods | MIT OR Apache-2.0 |
| [serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json) | Serialization, JSON export | MIT OR Apache-2.0 |
| [thiserror](https://github.com/dtolnay/thiserror) | Error types | MIT OR Apache-2.0 |
| [chrono](https://github.com/chronotope/chrono) | Date and time values | MIT OR Apache-2.0 |
| [rust_decimal](https://github.com/paupino/rust-decimal) | Decimal values | MIT |
| [uuid](https://github.com/uuid-rs/uuid) | UUID values, connection IDs | Apache-2.0 OR MIT |
| [csv](https://github.com/BurntSushi/rust-csv) | CSV import and export | Unlicense OR MIT |
| [rust_xlsxwriter](https://github.com/jmcnamara/rust_xlsxwriter) | Excel export | MIT OR Apache-2.0 |
| [image](https://github.com/image-rs/image) | PNG screenshots | MIT OR Apache-2.0 |
| [keyring](https://github.com/hwchen/keyring-rs) | OS keychain for passwords | MIT OR Apache-2.0 |
| [rfd](https://github.com/PolyMeilex/rfd) | Native file dialogs | MIT |
| [dirs](https://github.com/soc/dirs-rs) | OS configuration directories | MIT OR Apache-2.0 |

## Bundled and notable transitive components

| Component | Notes | License |
|---|---|---|
| [SQLite](https://sqlite.org) | Compiled in through `libsqlite3-sys` (bundled) | Public domain |
| [syntect](https://github.com/trishume/syntect) | SQL highlighting through `egui_extras` | MIT |
| [rustls](https://github.com/rustls/rustls), [ring](https://github.com/briansmith/ring), [aws-lc-rs](https://github.com/aws/aws-lc-rs) | TLS for database connections | Apache-2.0 / ISC / MIT (see each crate) |
| Fonts bundled by egui through `epaint_default_fonts` (Ubuntu-Light, Hack, Noto Emoji, emoji-icon-font, egui-icons) | UI text and icons | Ubuntu Font Licence 1.0, OFL-1.1, MIT, Bitstream Vera (see `epaint_default_fonts`) |
| [option-ext](https://crates.io/crates/option-ext) | Dependency of `dirs`; used unmodified | MPL-2.0 |

The full dependency tree has about 600 crates. Nearly all are under MIT, Apache-2.0, or both;
a few are under BSD, ISC, Zlib, Unicode-3.0, BSL-1.0 or CC0. Where a crate offers a choice that
includes a copyleft licence (for example `self_cell`: Apache-2.0 OR GPL-2.0-only, and `r-efi`:
MIT OR Apache-2.0 OR LGPL-2.1-or-later), DbCraft uses it under the permissive option. To list
every crate with its licence:

```sh
cargo metadata --format-version 1 | jq -r '.packages[] | "\(.name) \(.version): \(.license)"' | sort
```

## Trademarks

"Navicat" is a trademark of PremiumSoft CyberTech Ltd. DbCraft is not affiliated with it and uses
none of its code, artwork or assets. MySQL, MariaDB, PostgreSQL, Microsoft SQL Server and SQLite
are trademarks of their owners.
