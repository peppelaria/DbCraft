# db-drivers

Database backends for [DbCraft](../../README.md), implementing `db_core::Driver`.

| Engine | Library | Notes |
|---|---|---|
| SQLite | [sqlx](https://github.com/launchbadge/sqlx) (bundled SQLite) | File or `:memory:`; foreign keys on |
| PostgreSQL | sqlx | Schemas, identity columns, comments |
| MySQL / MariaDB | sqlx | `utf8mb4`; databases are shown as the second tree level |
| SQL Server | [tiberius](https://github.com/prisma/tiberius) | SQL authentication; schemas |

```rust
let drv = db_drivers::connect(&config, Some("my_database")).await?;
let results = drv.execute("SELECT 1; SELECT 2").await?; // one ResultSet per statement
```

Tests: `cargo test -p db-drivers` runs an end-to-end SQLite test. The PostgreSQL, MySQL and
SQL Server drivers do not have integration tests yet.

License: MIT OR Apache-2.0.
