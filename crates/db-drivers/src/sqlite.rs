use std::str::FromStr;

use async_trait::async_trait;
use db_core::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, TypeInfo, ValueRef};

use crate::common::*;

pub struct SqliteDriver {
    pool: SqlitePool,
}

fn decode(row: &SqliteRow, i: usize) -> Value {
    let Ok(raw) = row.try_get_raw(i) else { return Value::Null };
    if raw.is_null() {
        return Value::Null;
    }
    let ty = raw.type_info().name().to_ascii_uppercase();
    let v = match ty.as_str() {
        "INTEGER" => row.try_get_unchecked::<i64, _>(i).ok().map(Value::Int),
        "REAL" => row.try_get_unchecked::<f64, _>(i).ok().map(Value::Float),
        "BLOB" => row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(Value::Bytes),
        _ => None,
    };
    v.or_else(|| row.try_get_unchecked::<String, _>(i).ok().map(Value::Text))
        .or_else(|| row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(Value::Bytes))
        .unwrap_or(Value::Null)
}

impl SqliteDriver {
    pub async fn connect(cfg: &ConnectionConfig) -> DbResult<Self> {
        let path = cfg.database.trim();
        let memory = path.is_empty() || path == ":memory:";
        let opts = if memory {
            SqliteConnectOptions::from_str("sqlite::memory:").map_err(|e| DbError::Connection(e.to_string()))?
        } else {
            SqliteConnectOptions::new().filename(path).create_if_missing(true)
        };
        let pool = SqlitePoolOptions::new()
            .max_connections(if memory { 1 } else { 4 })
            .connect_with(opts.foreign_keys(true))
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        Ok(SqliteDriver { pool })
    }

    async fn q(&self, sql: &str) -> DbResult<ResultSet> {
        Ok(first(self.execute(sql).await?))
    }
}

#[async_trait]
impl Driver for SqliteDriver {
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let rs = self.q("PRAGMA database_list").await?;
        let mut v: Vec<String> = (0..rs.rows.len()).map(|r| text(&rs, r, 1)).filter(|n| n != "temp").collect();
        if v.is_empty() {
            v.push("main".into());
        }
        Ok(v)
    }

    async fn list_schemas(&self) -> DbResult<Vec<String>> {
        Ok(Vec::new())
    }

    async fn list_objects(&self, _schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
        let rs = self.q("SELECT name, type FROM sqlite_master WHERE type IN ('table','view','trigger') AND name NOT LIKE 'sqlite_%' ORDER BY name").await?;
        Ok((0..rs.rows.len())
            .map(|r| SchemaObject {
                name: text(&rs, r, 0),
                kind: match text(&rs, r, 1).as_str() {
                    "view" => ObjectKind::View,
                    "trigger" => ObjectKind::Trigger,
                    _ => ObjectKind::Table,
                },
                rows: None,
                comment: None,
            })
            .collect())
    }

    async fn describe_table(&self, _schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
        let d = Dialect::Sqlite;
        let lit = d.string_literal(table);
        let ddl = self.q(&format!("SELECT sql FROM sqlite_master WHERE name = {lit}")).await?;
        let autoinc = text(&ddl, 0, 0).to_ascii_uppercase().contains("AUTOINCREMENT");
        let cols = self.q(&format!("PRAGMA table_info({lit})")).await?;
        let pk_count = (0..cols.rows.len()).filter(|&r| text(&cols, r, 5) != "0").count();
        let columns = (0..cols.rows.len())
            .map(|r| {
                let pk = text(&cols, r, 5) != "0";
                ColumnInfo {
                    name: text(&cols, r, 1),
                    data_type: text(&cols, r, 2),
                    nullable: !truthy(&cols, r, 3),
                    default: opt_text(&cols, r, 4),
                    primary_key: pk,
                    auto_increment: pk && autoinc && pk_count == 1,
                    comment: None,
                }
            })
            .collect();
        let il = self.q(&format!("PRAGMA index_list({lit})")).await?;
        let mut indexes = Vec::new();
        for r in 0..il.rows.len() {
            if text(&il, r, 3) == "pk" {
                continue;
            }
            let name = text(&il, r, 1);
            let ii = self.q(&format!("PRAGMA index_info({})", d.string_literal(&name))).await?;
            indexes.push(IndexInfo { name, unique: truthy(&il, r, 2), columns: (0..ii.rows.len()).map(|k| text(&ii, k, 2)).collect() });
        }
        let fl = self.q(&format!("PRAGMA foreign_key_list({lit})")).await?;
        let mut foreign_keys: Vec<ForeignKeyInfo> = Vec::new();
        for r in 0..fl.rows.len() {
            let id = text(&fl, r, 0);
            let name = format!("fk_{table}_{id}");
            match foreign_keys.iter_mut().find(|f| f.name == name) {
                Some(f) => {
                    f.columns.push(text(&fl, r, 3));
                    f.ref_columns.push(text(&fl, r, 4));
                }
                None => foreign_keys.push(ForeignKeyInfo {
                    name,
                    columns: vec![text(&fl, r, 3)],
                    ref_table: text(&fl, r, 2),
                    ref_columns: vec![text(&fl, r, 4)],
                    on_update: opt_text(&fl, r, 5).filter(|s| s != "NO ACTION"),
                    on_delete: opt_text(&fl, r, 6).filter(|s| s != "NO ACTION"),
                }),
            }
        }
        Ok(TableInfo { schema: None, name: table.to_string(), columns, indexes, foreign_keys })
    }

    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        run_many(&self.pool, sql, decode, |r| r.rows_affected()).await
    }

    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64> {
        apply_in_tx(&self.pool, change_statements(Dialect::Sqlite, schema, table, changes), |r| r.rows_affected()).await
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}
