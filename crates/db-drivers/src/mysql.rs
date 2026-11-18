use async_trait::async_trait;
use db_core::*;
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow, MySqlSslMode};
use sqlx::{Row, TypeInfo, ValueRef};

use crate::common::*;

pub struct MySqlDriver {
    pool: MySqlPool,
}

fn decode(row: &MySqlRow, i: usize) -> Value {
    let Ok(raw) = row.try_get_raw(i) else { return Value::Null };
    if raw.is_null() {
        return Value::Null;
    }
    let ty = raw.type_info().name().to_ascii_uppercase();
    let base = ty.split_whitespace().next().unwrap_or("");
    let unsigned = ty.contains("UNSIGNED");
    let v = match base {
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" | "YEAR" if unsigned => {
            row.try_get_unchecked::<u64, _>(i).ok().map(|x| i64::try_from(x).map_or_else(|_| Value::Text(x.to_string()), Value::Int))
        }
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT" | "YEAR" => row.try_get_unchecked::<i64, _>(i).ok().map(Value::Int),
        "BOOLEAN" => row.try_get_unchecked::<bool, _>(i).ok().map(Value::Bool),
        "FLOAT" | "DOUBLE" => row.try_get_unchecked::<f64, _>(i).ok().map(Value::Float),
        // MySQL 8 reports catalog text (SHOW …, information_schema) as VARBINARY: show it as
        // text when it is printable UTF-8.
        "BINARY" | "VARBINARY" => row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(|b| match String::from_utf8(b) {
            Ok(s) if !s.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) => Value::Text(s),
            Ok(s) => Value::Bytes(s.into_bytes()),
            Err(e) => Value::Bytes(e.into_bytes()),
        }),
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "GEOMETRY" | "BIT" => row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(Value::Bytes),
        _ => None,
    };
    v.or_else(|| row.try_get_unchecked::<String, _>(i).ok().map(Value::Text))
        .or_else(|| row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(|b| Value::Text(String::from_utf8_lossy(&b).into_owned())))
        .unwrap_or(Value::Null)
}

/// MySQL reports literal defaults unquoted; the designer wants SQL expressions.
fn default_expr(v: Option<String>, data_type: &str) -> Option<String> {
    let v = v?;
    let upper = v.to_ascii_uppercase();
    let numeric = v.parse::<f64>().is_ok();
    let expr = upper.starts_with("CURRENT_") || upper == "NULL" || v.starts_with('\'') || v.contains('(');
    let numeric_type = ["int", "decimal", "float", "double", "bit"].iter().any(|t| data_type.contains(t));
    Some(if expr || (numeric && numeric_type) { v } else { Dialect::MySql.string_literal(&v) })
}

impl MySqlDriver {
    pub async fn connect(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Self> {
        let mut opts = MySqlConnectOptions::new()
            .host(if cfg.host.is_empty() { "localhost" } else { &cfg.host })
            .port(if cfg.port == 0 { 3306 } else { cfg.port })
            .username(&cfg.user)
            .password(&cfg.password)
            .charset("utf8mb4")
            .ssl_mode(match (cfg.ssl, cfg.tls.verify) {
                (false, _) => MySqlSslMode::Preferred,
                (true, TlsVerify::None) => MySqlSslMode::Required,
                (true, TlsVerify::Ca) => MySqlSslMode::VerifyCa,
                (true, TlsVerify::Full) => MySqlSslMode::VerifyIdentity,
            });
        if cfg.ssl {
            cfg.tls.check_files().map_err(DbError::Connection)?;
            if !cfg.tls.ca_cert.trim().is_empty() {
                opts = opts.ssl_ca(cfg.tls.ca_cert.trim());
            }
            if !cfg.tls.client_cert.trim().is_empty() {
                opts = opts.ssl_client_cert(cfg.tls.client_cert.trim()).ssl_client_key(cfg.tls.client_key.trim());
            }
        }
        let db = database.filter(|d| !d.is_empty()).or(Some(cfg.database.as_str()).filter(|d| !d.is_empty()));
        if let Some(db) = db {
            opts = opts.database(db);
        }
        let pool = MySqlPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(std::time::Duration::from_secs(15))
            .connect_with(opts)
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        Ok(MySqlDriver { pool })
    }

}

#[async_trait]
impl Driver for MySqlDriver {
    fn dialect(&self) -> Dialect {
        Dialect::MySql
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        list_databases(self).await
    }

    async fn list_schemas(&self) -> DbResult<Vec<String>> {
        list_schemas(self).await
    }

    async fn list_objects(&self, _schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
        list_objects(self, _schema).await
    }

    async fn describe_table(&self, _schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
        describe_table(self, _schema, table).await
    }

    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        run_many(&self.pool, sql, decode, |r| r.rows_affected()).await
    }

    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64> {
        apply_in_tx(&self.pool, change_statements(Dialect::MySql, schema, table, changes), |r| r.rows_affected()).await
    }

    async fn close(&self) {
        self.pool.close().await;
    }
}

// ----- introspection, shared with the HTTP tunnel driver -----

async fn q(d: &dyn Driver, sql: &str) -> DbResult<ResultSet> {
    Ok(first(d.execute(sql).await?))
}

pub(crate) async fn list_databases(d: &dyn Driver) -> DbResult<Vec<String>> {
    let rs = q(d, "SHOW DATABASES").await?;
    Ok((0..rs.rows.len()).map(|r| text(&rs, r, 0)).collect())
}

pub(crate) async fn list_schemas(_d: &dyn Driver) -> DbResult<Vec<String>> {
    Ok(Vec::new())
}

pub(crate) async fn list_objects(d: &dyn Driver, _schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
    let rs = q(d, "SELECT TABLE_NAME, IF(TABLE_TYPE = 'VIEW', 'V', 'T'), TABLE_ROWS, TABLE_COMMENT, AUTO_INCREMENT, \
              CAST(COALESCE(UPDATE_TIME, CREATE_TIME) AS CHAR), DATA_LENGTH, ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() \
            UNION ALL SELECT ROUTINE_NAME, IF(ROUTINE_TYPE = 'PROCEDURE', 'P', 'F'), NULL, ROUTINE_COMMENT, NULL, CAST(LAST_ALTERED AS CHAR), NULL, NULL FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = DATABASE() \
            UNION ALL SELECT TRIGGER_NAME, 'R', NULL, NULL, NULL, CAST(CREATED AS CHAR), NULL, NULL FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA = DATABASE() ORDER BY 1")
        .await?;
    Ok((0..rs.rows.len())
        .map(|r| SchemaObject {
            name: text(&rs, r, 0),
            kind: match text(&rs, r, 1).as_str() {
                "V" => ObjectKind::View,
                "P" => ObjectKind::Procedure,
                "F" => ObjectKind::Function,
                "R" => ObjectKind::Trigger,
                _ => ObjectKind::Table,
            },
            rows: rs.rows.get(r).and_then(|x| x.get(2)).and_then(Value::as_i64),
            comment: opt_text(&rs, r, 3).filter(|c| !c.is_empty() && c != "VIEW"),
            auto_increment: rs.rows.get(r).and_then(|x| x.get(4)).and_then(Value::as_i64),
            modified: opt_text(&rs, r, 5),
            data_length: rs.rows.get(r).and_then(|x| x.get(6)).and_then(Value::as_i64),
            engine: opt_text(&rs, r, 7),
        })
        .collect())
}

pub(crate) async fn describe_table(d: &dyn Driver, _schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
    let t = Dialect::MySql.string_literal(table);
    let cols = q(d, &format!(
            "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_KEY, EXTRA, COLUMN_COMMENT FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = {t} ORDER BY ORDINAL_POSITION"
        ))
        .await?;
    let columns = (0..cols.rows.len())
        .map(|r| {
            let data_type = text(&cols, r, 1);
            ColumnInfo {
                name: text(&cols, r, 0),
                nullable: text(&cols, r, 2) == "YES",
                default: default_expr(opt_text(&cols, r, 3), &data_type),
                primary_key: text(&cols, r, 4) == "PRI",
                auto_increment: text(&cols, r, 5).contains("auto_increment"),
                comment: opt_text(&cols, r, 6).filter(|c| !c.is_empty()),
                data_type,
            }
        })
        .collect();
    let idx = q(d, &format!(
            "SELECT INDEX_NAME, MIN(NON_UNIQUE), GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = {t} AND INDEX_NAME <> 'PRIMARY' GROUP BY INDEX_NAME ORDER BY 1"
        ))
        .await?;
    let mut indexes: Vec<IndexInfo> = (0..idx.rows.len()).map(|r| IndexInfo { name: text(&idx, r, 0), unique: !truthy(&idx, r, 1), columns: split_list(&text(&idx, r, 2)) }).collect();
    let fk = q(d, &format!(
            "SELECT k.CONSTRAINT_NAME, GROUP_CONCAT(k.COLUMN_NAME ORDER BY k.ORDINAL_POSITION), k.REFERENCED_TABLE_NAME, \
             GROUP_CONCAT(k.REFERENCED_COLUMN_NAME ORDER BY k.ORDINAL_POSITION), r.DELETE_RULE, r.UPDATE_RULE \
             FROM information_schema.KEY_COLUMN_USAGE k JOIN information_schema.REFERENTIAL_CONSTRAINTS r \
             ON r.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA AND r.CONSTRAINT_NAME = k.CONSTRAINT_NAME \
             WHERE k.TABLE_SCHEMA = DATABASE() AND k.TABLE_NAME = {t} AND k.REFERENCED_TABLE_NAME IS NOT NULL \
             GROUP BY k.CONSTRAINT_NAME, k.REFERENCED_TABLE_NAME, r.DELETE_RULE, r.UPDATE_RULE ORDER BY 1"
        ))
        .await?;
    let foreign_keys = (0..fk.rows.len())
        .map(|r| ForeignKeyInfo {
            name: text(&fk, r, 0),
            columns: split_list(&text(&fk, r, 1)),
            ref_table: text(&fk, r, 2),
            ref_columns: split_list(&text(&fk, r, 3)),
            on_delete: opt_text(&fk, r, 4).filter(|a| a != "NO ACTION" && a != "RESTRICT"),
            on_update: opt_text(&fk, r, 5).filter(|a| a != "NO ACTION" && a != "RESTRICT"),
        })
        .collect::<Vec<ForeignKeyInfo>>();
    // MySQL creates an index named after a foreign key that has none to use: it belongs to the
    // key, not to the designer's index list.
    indexes.retain(|i| !foreign_keys.iter().any(|f| f.name == i.name));
    Ok(TableInfo { schema: None, name: table.to_string(), columns, indexes, foreign_keys })
}
