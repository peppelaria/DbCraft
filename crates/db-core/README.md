# db-core

Core types for [DbCraft](../../README.md), independent of any UI and of any database driver.

- `Value`, `ResultSet`, `ColumnMeta`: database values and query results.
- `TableInfo`, `ColumnInfo`, `IndexInfo`, `ForeignKeyInfo`, `SchemaObject`: schema metadata.
- `ConnectionConfig`, `DbKind`: saved connection settings. The password is never serialised.
- `Driver`: the async trait every backend implements (list databases, schemas and objects;
  describe a table; execute SQL; fetch a page; apply row changes in a transaction).
- `Dialect`: per-engine SQL generation (SQLite, PostgreSQL, MySQL/MariaDB, SQL Server).
  It covers identifier quoting, literals, paging, `INSERT`/`UPDATE`/`DELETE` from `RowChange`,
  and `CREATE TABLE` / `ALTER TABLE` from a `TableDesign`. On SQLite, `ALTER TABLE` falls back
  to rebuilding the table and copying its data.

License: MIT OR Apache-2.0.
