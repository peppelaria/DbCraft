//! Database drivers for DbCraft: SQLite, PostgreSQL and MySQL/MariaDB through `sqlx`,
//! SQL Server through `tiberius`, optionally reached through an SSH tunnel.

mod common;
mod mssql;
mod mysql;
mod postgres;
mod sqlite;
pub mod ssh;

use std::sync::Arc;

use db_core::{ConnectionConfig, DbKind, DbResult, Driver};

/// Opens a connection. `database` overrides the configured initial database.
pub async fn connect(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Arc<dyn Driver>> {
    if cfg.ssh.enabled && cfg.kind != DbKind::Sqlite {
        // The database host is resolved by the SSH server ("localhost" = the SSH machine).
        let target_host = if cfg.host.trim().is_empty() { "localhost" } else { cfg.host.trim() };
        let target_port = if cfg.port == 0 { cfg.kind.default_port() } else { cfg.port };
        let tunnel = ssh::open(&cfg.ssh, target_host, target_port).await?;
        let mut local = cfg.clone();
        local.host = "127.0.0.1".into();
        local.port = tunnel.local_port;
        let inner = connect_direct(&local, database).await?;
        return Ok(Arc::new(ssh::TunneledDriver { inner, tunnel }));
    }
    connect_direct(cfg, database).await
}

async fn connect_direct(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Arc<dyn Driver>> {
    Ok(match cfg.kind {
        DbKind::Sqlite => Arc::new(sqlite::SqliteDriver::connect(cfg).await?),
        DbKind::Postgres => Arc::new(postgres::PgDriver::connect(cfg, database).await?),
        DbKind::MySql | DbKind::MariaDb => Arc::new(mysql::MySqlDriver::connect(cfg, database).await?),
        DbKind::SqlServer => Arc::new(mssql::MssqlDriver::connect(cfg, database).await?),
    })
}
