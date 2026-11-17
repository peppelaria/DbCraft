//! Database drivers for DbCraft: SQLite, PostgreSQL and MySQL/MariaDB through `sqlx`,
//! SQL Server through `tiberius`.

mod common;
mod mssql;
mod mysql;
mod postgres;
mod sqlite;

use std::sync::Arc;

use db_core::{ConnectionConfig, DbKind, DbResult, Driver};

/// Opens a connection. `database` overrides the configured initial database.
pub async fn connect(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Arc<dyn Driver>> {
    Ok(match cfg.kind {
        DbKind::Sqlite => Arc::new(sqlite::SqliteDriver::connect(cfg).await?),
        DbKind::Postgres => Arc::new(postgres::PgDriver::connect(cfg, database).await?),
        DbKind::MySql | DbKind::MariaDb => Arc::new(mysql::MySqlDriver::connect(cfg, database).await?),
        DbKind::SqlServer => Arc::new(mssql::MssqlDriver::connect(cfg, database).await?),
    })
}
