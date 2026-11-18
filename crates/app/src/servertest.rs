//! Helpers for the tests against the servers of `docker-compose.test.yml`
//! (run with `DBCRAFT_TEST_SERVERS=1 cargo test -p dbcraft -- --test-threads=1`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use db_core::{ConnectionConfig, DbKind, Driver};

pub fn enabled() -> bool {
    std::env::var("DBCRAFT_TEST_SERVERS").is_ok_and(|v| v == "1")
}

pub fn cfg(kind: DbKind) -> ConnectionConfig {
    let (port, database) = match kind {
        DbKind::Postgres => (55432, "dbcraft"),
        DbKind::MySql => (53306, "dbcraft"),
        DbKind::MariaDb => (53307, "dbcraft"),
        DbKind::SqlServer => (51433, "master"),
        DbKind::Sqlite => (0, ""),
    };
    ConnectionConfig {
        id: format!("test-{kind:?}"),
        name: kind.label().into(),
        kind,
        host: "127.0.0.1".into(),
        port,
        user: kind.default_user().into(),
        password: "DbCraft_test1".into(),
        database: database.into(),
        ..Default::default()
    }
}

/// A driver on a scratch place for `kind`: (driver, schema). MySQL/MariaDB get their own
/// database `name`, PostgreSQL/SQL Server a schema `name` (SQL Server in the `dbcraft` database),
/// SQLite a file in a temporary folder.
pub async fn scratch(kind: DbKind, name: &str, dir: &std::path::Path) -> (Arc<dyn Driver>, Option<String>, ConnectionConfig) {
    match kind {
        DbKind::Sqlite => {
            let c = ConnectionConfig {
                id: "test-sqlite".into(),
                name: "sqlite".into(),
                kind,
                database: dir.join(format!("{name}.db")).to_string_lossy().into_owned(),
                ..Default::default()
            };
            (db_drivers::connect(&c, None).await.unwrap(), None, c)
        }
        DbKind::MySql | DbKind::MariaDb => {
            let c = cfg(kind);
            let root = db_drivers::connect(&c, None).await.unwrap();
            root.execute(&format!("DROP DATABASE IF EXISTS `{name}`; CREATE DATABASE `{name}`")).await.unwrap();
            (db_drivers::connect(&c, Some(name)).await.unwrap(), None, c)
        }
        DbKind::Postgres => {
            let c = cfg(kind);
            let d = db_drivers::connect(&c, None).await.unwrap();
            d.execute(&format!("DROP SCHEMA IF EXISTS {name} CASCADE; CREATE SCHEMA {name}")).await.unwrap();
            (d, Some(name.to_string()), c)
        }
        DbKind::SqlServer => {
            let c = cfg(kind);
            let master = db_drivers::connect(&c, None).await.unwrap();
            master.execute("IF DB_ID('dbcraft') IS NULL CREATE DATABASE dbcraft").await.unwrap();
            let d = db_drivers::connect(&c, Some("dbcraft")).await.unwrap();
            // drop the schema's objects, then the schema
            let objs = d
                .execute(&format!(
                    "SELECT o.name, o.type FROM sys.objects o WHERE o.schema_id = SCHEMA_ID('{name}') AND o.type IN ('V','U') ORDER BY CASE o.type WHEN 'V' THEN 0 ELSE 1 END"
                ))
                .await
                .unwrap();
            for _ in 0..3 {
                for r in &objs[0].rows {
                    let (n, t) = (r[0].to_string(), r[1].to_string());
                    let what = if t.trim() == "V" { "VIEW" } else { "TABLE" };
                    let _ = d.execute(&format!("DROP {what} [{name}].[{n}]")).await;
                }
            }
            d.execute(&format!("IF SCHEMA_ID('{name}') IS NOT NULL DROP SCHEMA [{name}]")).await.unwrap();
            d.execute(&format!("CREATE SCHEMA [{name}]")).await.unwrap();
            (d, Some(name.to_string()), c)
        }
    }
}

pub const ALL: [DbKind; 5] = [DbKind::Sqlite, DbKind::MySql, DbKind::MariaDb, DbKind::Postgres, DbKind::SqlServer];
