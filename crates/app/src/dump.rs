//! Backup and restore of a database (or schema) as an SQL script, like Navicat's Backup: the
//! tables (structure and data, in foreign-key order) and the views, written so that the script
//! can be run again on the same kind of server ("Ripristina").

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use db_core::{Dialect, Driver, ObjectKind, PageRequest, TableDesign, TableInfo, Value, split::split_statements};

/// Rows read per page while dumping a table.
const PAGE: u64 = 1000;

fn text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::Null => None,
        Value::Text(t) => Some(t.clone()),
        // MySQL gives some catalog texts (SHOW CREATE TABLE) as bytes
        Value::Bytes(b) => Some(String::from_utf8_lossy(b).into_owned()),
        other => Some(other.to_string()),
    }
}

async fn one(d: &dyn Driver, sql: &str) -> Result<Option<String>, String> {
    let rs = d.execute(sql).await.map_err(|e| e.to_string())?;
    Ok(rs.first().and_then(|r| r.rows.first()).and_then(|row| text(row.first())))
}

async fn cell(d: &dyn Driver, sql: &str, col: usize) -> Result<Option<String>, String> {
    let rs = d.execute(sql).await.map_err(|e| e.to_string())?;
    Ok(rs.first().and_then(|r| r.rows.first()).and_then(|row| text(row.get(col))))
}

/// Tables in an order where every table comes after the tables it references.
fn fk_order(tables: Vec<TableInfo>) -> Vec<TableInfo> {
    let names: Vec<String> = tables.iter().map(|t| t.name.clone()).collect();
    let mut left = tables;
    let mut out: Vec<TableInfo> = Vec::new();
    while !left.is_empty() {
        let done: Vec<String> = out.iter().map(|t| t.name.clone()).collect();
        let ready = left.iter().position(|t| t.foreign_keys.iter().all(|f| f.ref_table == t.name || done.contains(&f.ref_table) || !names.contains(&f.ref_table)));
        // a cycle: take the next one anyway (the checks are off while restoring on MySQL/SQLite)
        let i = ready.unwrap_or(0);
        out.push(left.remove(i));
    }
    out
}

/// The statements creating a table.
async fn create_sql(d: &dyn Driver, dialect: Dialect, schema: Option<&str>, t: &TableInfo) -> Result<Vec<String>, String> {
    match dialect {
        // the server's own DDL: exact defaults, charsets, engines…
        Dialect::MySql => {
            let ddl = cell(d, &format!("SHOW CREATE TABLE {}", dialect.quote(&t.name)), 1).await?;
            Ok(vec![ddl.ok_or_else(|| format!("{}: struttura non trovata", t.name))?])
        }
        Dialect::Sqlite => {
            let rs = d
                .execute(&format!(
                    "SELECT sql FROM sqlite_master WHERE tbl_name = {} AND type IN ('table','index') AND sql IS NOT NULL ORDER BY type = 'index'",
                    dialect.string_literal(&t.name)
                ))
                .await
                .map_err(|e| e.to_string())?;
            Ok(rs.first().map(|r| r.rows.iter().filter_map(|row| text(row.first())).collect()).unwrap_or_default())
        }
        Dialect::Postgres | Dialect::SqlServer => {
            let mut design = TableDesign::from_info(t);
            design.schema = schema.map(String::from);
            for c in &mut design.columns {
                // a serial's sequence goes with the table: an identity column in its place
                if c.default.trim_start().starts_with("nextval(") {
                    c.default.clear();
                    c.auto_increment = true;
                }
                if dialect == Dialect::SqlServer && c.auto_increment {
                    c.default.clear();
                }
            }
            Ok(dialect.create_table(&design))
        }
    }
}

/// The statement creating a view, if the server tells its definition.
async fn view_sql(d: &dyn Driver, dialect: Dialect, schema: Option<&str>, name: &str) -> Result<Option<String>, String> {
    let lit = |s: &str| dialect.string_literal(s);
    let q = dialect.qualified(schema, name);
    Ok(match dialect {
        Dialect::Sqlite => one(d, &format!("SELECT sql FROM sqlite_master WHERE type = 'view' AND name = {}", lit(name))).await?,
        Dialect::MySql => one(d, &format!("SELECT VIEW_DEFINITION FROM information_schema.VIEWS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = {}", lit(name)))
            .await?
            .map(|def| format!("CREATE VIEW {q} AS {def}")),
        Dialect::Postgres => {
            one(d, &format!("SELECT pg_get_viewdef({}::regclass, true)", lit(&q))).await?.map(|def| format!("CREATE VIEW {q} AS\n{}", def.trim().trim_end_matches(';')))
        }
        Dialect::SqlServer => one(d, &format!("SELECT OBJECT_DEFINITION(OBJECT_ID({}))", lit(&q))).await?.map(|def| def.trim().trim_end_matches(';').to_string()),
    })
}

/// Writes the backup of the database the driver is on (`schema` for PostgreSQL / SQL Server) to
/// `path`. Returns a summary.
pub async fn backup(d: Arc<dyn Driver>, schema: Option<String>, path: &Path, progress: &(dyn Fn(String) + Send + Sync)) -> Result<String, String> {
    let dialect = d.dialect();
    let schema = schema.as_deref();
    let objects = d.list_objects(schema).await.map_err(|e| e.to_string())?;
    let mut infos = Vec::new();
    for o in objects.iter().filter(|o| o.kind == ObjectKind::Table) {
        progress(format!("Leggo la struttura di {}…", o.name));
        infos.push(d.describe_table(schema, &o.name).await.map_err(|e| format!("{}: {e}", o.name))?);
    }
    let tables = fk_order(infos);
    let views: Vec<String> = objects.iter().filter(|o| o.kind == ObjectKind::View).map(|o| o.name.clone()).collect();

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("part");
    let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let db_name = match dialect {
        Dialect::MySql => one(d.as_ref(), "SELECT DATABASE()").await.ok().flatten(),
        Dialect::Postgres => one(d.as_ref(), "SELECT current_database()").await.ok().flatten(),
        Dialect::SqlServer => one(d.as_ref(), "SELECT DB_NAME()").await.ok().flatten(),
        Dialect::Sqlite => Some("main".into()),
    }
    .unwrap_or_default();
    let place = match schema {
        Some(s) => format!("{db_name}, schema {s}"),
        None => db_name,
    };
    let mut w = std::io::BufWriter::new(file);
    let mut out = |s: &str| -> Result<(), String> { w.write_all(s.as_bytes()).map_err(|e| e.to_string()) };
    out(&format!(
        "-- DbCraft backup\n-- Server: {:?}\n-- Database: {}\n-- Data: {}\n-- Tabelle: {}, viste: {}\n\n",
        dialect,
        place,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        tables.len(),
        views.len()
    ))?;
    match dialect {
        Dialect::MySql => out("SET FOREIGN_KEY_CHECKS = 0;\n\n")?,
        Dialect::Sqlite => out("PRAGMA foreign_keys = OFF;\n\n")?,
        _ => {}
    }
    // drop what is there: views first, then the tables from the last referencing ones
    for v in &views {
        out(&format!("DROP VIEW IF EXISTS {};\n", dialect.qualified(schema, v)))?;
    }
    for t in tables.iter().rev() {
        out(&format!("DROP TABLE IF EXISTS {};\n", dialect.qualified(schema, &t.name)))?;
    }
    out("\n")?;
    let mut total_rows = 0u64;
    for t in &tables {
        out(&format!("-- ----------------------------\n-- Tabella {}\n-- ----------------------------\n", t.name))?;
        for s in create_sql(d.as_ref(), dialect, schema, t).await? {
            out(&format!("{};\n", s.trim().trim_end_matches(';')))?;
        }
        out("\n")?;
        let cols: Vec<String> = t.columns.iter().map(|c| c.name.clone()).collect();
        let identity = dialect == Dialect::SqlServer && t.columns.iter().any(|c| c.auto_increment);
        if identity {
            out(&format!("SET IDENTITY_INSERT {} ON;\n", dialect.qualified(schema, &t.name)))?;
        }
        let order: Vec<(String, bool)> = t.primary_key().into_iter().map(|c| (c, false)).collect();
        let mut offset = 0u64;
        loop {
            let req = PageRequest { schema: schema.map(String::from), table: t.name.clone(), filter: None, order: order.clone(), offset, limit: PAGE };
            let page = d.fetch_page(&req).await.map_err(|e| format!("{}: {e}", t.name))?;
            let n = page.rows.len() as u64;
            let names: Vec<String> = if page.columns.is_empty() { cols.clone() } else { page.columns.iter().map(|c| c.name.clone()).collect() };
            for row in &page.rows {
                out(&format!("{};\n", dialect.insert_sql(schema, &t.name, &names, row)))?;
            }
            total_rows += n;
            offset += n;
            progress(format!("{}: {offset} record…", t.name));
            if n < PAGE {
                break;
            }
        }
        if identity {
            out(&format!("SET IDENTITY_INSERT {} OFF;\n", dialect.qualified(schema, &t.name)))?;
        }
        if dialect == Dialect::Postgres {
            // the identity goes on from the highest value restored
            for c in t.columns.iter().filter(|c| c.auto_increment || c.default.as_deref().is_some_and(|x| x.starts_with("nextval("))) {
                let q = dialect.qualified(schema, &t.name);
                out(&format!(
                    "SELECT setval(pg_get_serial_sequence({}, {}), COALESCE((SELECT MAX({}) FROM {q}), 0) + 1, false);\n",
                    dialect.string_literal(&q),
                    dialect.string_literal(&c.name),
                    dialect.quote(&c.name)
                ))?;
            }
        }
        out("\n")?;
    }
    for v in &views {
        if let Some(sql) = view_sql(d.as_ref(), dialect, schema, v).await? {
            out(&format!("-- Vista {v}\n{};\n\n", sql.trim()))?;
        }
    }
    match dialect {
        Dialect::MySql => out("SET FOREIGN_KEY_CHECKS = 1;\n")?,
        Dialect::Sqlite => out("PRAGMA foreign_keys = ON;\n")?,
        _ => {}
    }
    w.flush().map_err(|e| e.to_string())?;
    drop(w);
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(format!("Backup completato: {} tabelle, {} viste, {total_rows} record", tables.len(), views.len()))
}

/// Runs a backup script (or any SQL script) on the database the driver is on.
pub async fn restore(d: Arc<dyn Driver>, path: &Path, progress: &(dyn Fn(String) + Send + Sync)) -> Result<String, String> {
    let dialect = d.dialect();
    let sql = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let stmts: Vec<String> = split_statements(&sql, dialect).into_iter().filter(|s| !s.trim().is_empty()).collect();
    // the statements go in batches: one call runs on one connection, so the session settings
    // (foreign keys off) are repeated at the start of each batch
    // …and switched back on at the end: the pool gives the connection to other jobs afterwards
    let (prelude, epilogue) = match dialect {
        Dialect::MySql => ("SET FOREIGN_KEY_CHECKS = 0;\n", "SET FOREIGN_KEY_CHECKS = 1;\n"),
        Dialect::Sqlite => ("PRAGMA foreign_keys = OFF;\n", "PRAGMA foreign_keys = ON;\n"),
        _ => ("", ""),
    };
    let batch = if dialect == Dialect::SqlServer { 1 } else { 200 };
    let total = stmts.len();
    for (i, chunk) in stmts.chunks(batch).enumerate() {
        let mut text = String::from(prelude);
        for s in chunk {
            text.push_str(s.trim());
            text.push_str(";\n");
        }
        text.push_str(epilogue);
        let res = d.execute(&text).await;
        if res.is_err() && !epilogue.is_empty() {
            let _ = d.execute(epilogue).await;
        }
        res.map_err(|e| {
            let first = chunk.first().map(|s| s.lines().next().unwrap_or("").chars().take(80).collect::<String>()).unwrap_or_default();
            format!("istruzione {} ({first}…): {e}", i * batch + 1)
        })?;
        progress(format!("Ripristino: {} di {total} istruzioni…", ((i + 1) * batch).min(total)));
    }
    Ok(format!("Ripristino completato: {total} istruzioni eseguite"))
}

/// Starts a backup of `loc` into its backup folder, in the background. Progress and outcome
/// come back as [`Msg::DumpProgress`] / [`Msg::Dumped`] for the tab `tab` or the scheduled job `job`.
pub fn start_backup(
    backend: &crate::backend::Backend,
    cfg: &db_core::ConnectionConfig,
    loc: &crate::app::Location,
    tab: Option<crate::backend::TabId>,
    job: Option<u64>,
) -> std::path::PathBuf {
    use crate::backend::Msg;
    let dir = crate::store::place_dir("backups", &loc.conn, &loc.db, loc.schema.as_deref());
    let path = dir.join(format!("{}.sql", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));
    let notifier = backend.notifier();
    let schema = loc.schema.clone();
    let p = path.clone();
    backend.with_driver(cfg, &loc.db, move |d| async move {
        let progress = move |text: String| notifier.send(Msg::DumpProgress { tab, text });
        let res = match d {
            Ok(d) => backup(d, schema, &p, &progress).await,
            Err(e) => Err(e.to_string()),
        };
        if res.is_err() {
            let _ = std::fs::remove_file(p.with_extension("part"));
        }
        Msg::Dumped { tab, job, res }
    });
    path
}

/// Restores the backup `path` on `loc`, in the background.
pub fn start_restore(backend: &crate::backend::Backend, cfg: &db_core::ConnectionConfig, loc: &crate::app::Location, tab: crate::backend::TabId, path: std::path::PathBuf) {
    use crate::backend::Msg;
    let notifier = backend.notifier();
    backend.with_driver(cfg, &loc.db, move |d| async move {
        let progress = move |text: String| notifier.send(Msg::DumpProgress { tab: Some(tab), text });
        let res = match d {
            Ok(d) => restore(d, &path, &progress).await,
            Err(e) => Err(e.to_string()),
        };
        Msg::Dumped { tab: Some(tab), job: None, res }
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use db_core::ForeignKeyInfo;

    #[test]
    fn tables_after_their_references() {
        let t = |n: &str, refs: &[&str]| TableInfo {
            name: n.into(),
            foreign_keys: refs.iter().map(|r| ForeignKeyInfo { ref_table: (*r).into(), ..Default::default() }).collect(),
            ..Default::default()
        };
        let order: Vec<String> =
            fk_order(vec![t("orders", &["customers"]), t("items", &["orders", "products"]), t("customers", &[]), t("products", &[])]).into_iter().map(|t| t.name).collect();
        let pos = |n: &str| order.iter().position(|x| x == n).unwrap();
        assert!(pos("customers") < pos("orders"));
        assert!(pos("orders") < pos("items"));
        assert!(pos("products") < pos("items"));
    }

    /// Backup, damage, restore and compare, on every server.
    #[test]
    fn backup_and_restore_on_servers() {
        if !crate::servertest::enabled() {
            return;
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tmp = std::env::temp_dir().join(format!("dbcraft-dump-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        for kind in crate::servertest::ALL {
            rt.block_on(async {
                let (d, schema, _) = crate::servertest::scratch(kind, "dump_t", &tmp).await;
                let dl = d.dialect();
                let q = |n: &str| dl.qualified(schema.as_deref(), n);
                let (id, text, money) = match dl {
                    Dialect::MySql => ("INT AUTO_INCREMENT PRIMARY KEY", "VARCHAR(100) DEFAULT 'nuovo'", "DECIMAL(10,2)"),
                    Dialect::Postgres => ("SERIAL PRIMARY KEY", "VARCHAR(100) DEFAULT 'nuovo'", "NUMERIC(10,2)"),
                    Dialect::SqlServer => ("INT IDENTITY(1,1) PRIMARY KEY", "NVARCHAR(100) DEFAULT 'nuovo'", "DECIMAL(10,2)"),
                    Dialect::Sqlite => ("INTEGER PRIMARY KEY AUTOINCREMENT", "TEXT DEFAULT 'nuovo'", "NUMERIC"),
                };
                let setup = [
                    format!("CREATE TABLE {} (id {id}, name {text}, note {text})", q("customers")),
                    format!("CREATE TABLE {} (id {id}, customer_id INT NOT NULL, total {money}, FOREIGN KEY (customer_id) REFERENCES {}(id))", q("orders"), q("customers")),
                    format!("INSERT INTO {} (name, note) VALUES ('Rossi', 'l''amico'), ('Bianchi', NULL), ('Müller ✓', 'a;b')", q("customers")),
                    format!("INSERT INTO {} (customer_id, total) VALUES (1, 10.5), (1, 20), (3, 7.25)", q("orders")),
                    format!("CREATE VIEW {} AS SELECT c.name, o.total FROM {} c JOIN {} o ON o.customer_id = c.id", q("big_orders"), q("customers"), q("orders")),
                ];
                for sql in &setup {
                    d.execute(sql).await.unwrap_or_else(|e| panic!("{kind:?}: {sql}: {e}"));
                }
                let file = tmp.join(format!("{kind:?}.sql"));
                let msg = backup(d.clone(), schema.clone(), &file, &|_| {}).await.unwrap_or_else(|e| panic!("{kind:?} backup: {e}"));
                assert!(msg.contains("2 tabelle, 1 viste, 6 record"), "{kind:?}: {msg}");
                // damage: rows gone, the view gone
                d.execute(&format!("DROP VIEW {}", q("big_orders"))).await.unwrap();
                d.execute(&format!("DELETE FROM {}", q("orders"))).await.unwrap();
                restore(d.clone(), &file, &|_| {}).await.unwrap_or_else(|e| panic!("{kind:?} restore: {e}\n{}", std::fs::read_to_string(&file).unwrap_or_default()));
                let count = |t: &str| format!("SELECT COUNT(*) FROM {}", q(t));
                for (t, n) in [("customers", 3), ("orders", 3), ("big_orders", 3)] {
                    let rs = d.execute(&count(t)).await.unwrap();
                    assert_eq!(rs[0].rows[0][0].as_i64(), Some(n), "{kind:?} {t}");
                }
                let rs = d.execute(&format!("SELECT note FROM {} WHERE id = 1", q("customers"))).await.unwrap();
                assert_eq!(rs[0].rows[0][0].to_string(), "l'amico", "{kind:?}");
                // the ids go on after the restored ones
                d.execute(&format!("INSERT INTO {} (name) VALUES ('Verdi')", q("customers"))).await.unwrap_or_else(|e| panic!("{kind:?} insert after restore: {e}"));
                let rs = d.execute(&format!("SELECT id, note FROM {} WHERE name = 'Verdi'", q("customers"))).await.unwrap();
                assert_eq!(rs[0].rows[0][0].as_i64(), Some(4), "{kind:?}");
                assert_eq!(rs[0].rows[0][1].to_string(), "nuovo", "{kind:?}: default");
                // the foreign key is back
                assert!(d.execute(&format!("INSERT INTO {} (customer_id, total) VALUES (999, 1)", q("orders"))).await.is_err() || kind == db_core::DbKind::Sqlite, "{kind:?}: FK");
                d.close().await;
            });
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
