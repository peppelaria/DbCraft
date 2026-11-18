use async_trait::async_trait;
use db_core::*;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow, PgSslMode};
use sqlx::{Row, TypeInfo, ValueRef};

use crate::common::*;

pub struct PgDriver {
    pool: PgPool,
}

fn decode(row: &PgRow, i: usize) -> Value {
    let Ok(raw) = row.try_get_raw(i) else { return Value::Null };
    if raw.is_null() {
        return Value::Null;
    }
    let ty = raw.type_info().name().to_ascii_uppercase();
    let v = match ty.as_str() {
        "INT2" => row.try_get::<i16, _>(i).ok().map(|x| Value::Int(x.into())),
        "INT4" => row.try_get::<i32, _>(i).ok().map(|x| Value::Int(x.into())),
        "INT8" => row.try_get::<i64, _>(i).ok().map(Value::Int),
        "FLOAT4" => row.try_get::<f32, _>(i).ok().map(|x| Value::Float(x.into())),
        "FLOAT8" => row.try_get::<f64, _>(i).ok().map(Value::Float),
        "BOOL" => row.try_get::<bool, _>(i).ok().map(Value::Bool),
        "BYTEA" => row.try_get::<Vec<u8>, _>(i).ok().map(Value::Bytes),
        _ => None,
    };
    v.or_else(|| row.try_get_unchecked::<String, _>(i).ok().map(Value::Text))
        .or_else(|| row.try_get_unchecked::<Vec<u8>, _>(i).ok().map(|b| Value::Text(String::from_utf8_lossy(&b).into_owned())))
        .unwrap_or(Value::Null)
}

fn fk_action(c: &str) -> Option<String> {
    match c {
        "r" => Some("RESTRICT".into()),
        "c" => Some("CASCADE".into()),
        "n" => Some("SET NULL".into()),
        "d" => Some("SET DEFAULT".into()),
        _ => None,
    }
}

impl PgDriver {
    pub async fn connect(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Self> {
        let db = database.map(str::to_string).filter(|d| !d.is_empty()).unwrap_or_else(|| if cfg.database.is_empty() { "postgres".into() } else { cfg.database.clone() });
        let opts = PgConnectOptions::new()
            .host(if cfg.host.is_empty() { "localhost" } else { &cfg.host })
            .port(if cfg.port == 0 { 5432 } else { cfg.port })
            .username(&cfg.user)
            .password(&cfg.password)
            .database(&db)
            .application_name("DbCraft")
            .ssl_mode(match (cfg.ssl, cfg.tls.verify) {
                (false, _) => PgSslMode::Prefer,
                (true, TlsVerify::None) => PgSslMode::Require,
                (true, TlsVerify::Ca) => PgSslMode::VerifyCa,
                (true, TlsVerify::Full) => PgSslMode::VerifyFull,
            });
        let mut opts = opts;
        if cfg.ssl {
            cfg.tls.check_files().map_err(DbError::Connection)?;
            if !cfg.tls.ca_cert.trim().is_empty() {
                opts = opts.ssl_root_cert(cfg.tls.ca_cert.trim());
            }
            if !cfg.tls.client_cert.trim().is_empty() {
                opts = opts.ssl_client_cert(cfg.tls.client_cert.trim()).ssl_client_key(cfg.tls.client_key.trim());
            }
        }
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(std::time::Duration::from_secs(15))
            .connect_with(opts)
            .await
            .map_err(|e| DbError::Connection(e.to_string()))?;
        Ok(PgDriver { pool })
    }

}

#[async_trait]
impl Driver for PgDriver {
    fn dialect(&self) -> Dialect {
        Dialect::Postgres
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        list_databases(self).await
    }

    async fn list_schemas(&self) -> DbResult<Vec<String>> {
        list_schemas(self).await
    }

    async fn list_objects(&self, schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
        list_objects(self, schema).await
    }

    async fn describe_table(&self, schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
        describe_table(self, schema, table).await
    }

    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        run_many(&self.pool, sql, decode, |r| r.rows_affected()).await
    }

    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64> {
        apply_in_tx(&self.pool, change_statements(Dialect::Postgres, schema, table, changes), |r| r.rows_affected()).await
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
    let rs = q(d, "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY 1").await?;
    Ok((0..rs.rows.len()).map(|r| text(&rs, r, 0)).collect())
}

pub(crate) async fn list_schemas(d: &dyn Driver) -> DbResult<Vec<String>> {
    let rs = q(d, "SELECT nspname FROM pg_namespace WHERE nspname NOT IN ('pg_catalog','information_schema') AND nspname NOT LIKE 'pg\\_toast%' AND nspname NOT LIKE 'pg\\_temp%' ORDER BY 1")
        .await?;
    Ok((0..rs.rows.len()).map(|r| text(&rs, r, 0)).collect())
}

pub(crate) async fn list_objects(d: &dyn Driver, schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
    let s = Dialect::Postgres.string_literal(schema.unwrap_or("public"));
    let sql = format!(
        "SELECT c.relname, CASE WHEN c.relkind IN ('v','m') THEN 'V' ELSE 'T' END, c.reltuples::bigint, obj_description(c.oid, 'pg_class'), \
           CASE WHEN c.relkind IN ('r','p','m') THEN pg_relation_size(c.oid) END \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = {s} AND c.relkind IN ('r','p','v','m','f') \
         UNION ALL SELECT p.proname, CASE WHEN p.prokind = 'p' THEN 'P' ELSE 'F' END, NULL::bigint, NULL::text, NULL::bigint FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace WHERE n.nspname = {s} \
         UNION ALL SELECT DISTINCT trigger_name::text, 'R', NULL::bigint, NULL::text, NULL::bigint FROM information_schema.triggers WHERE trigger_schema = {s} ORDER BY 1"
    );
    let rs = q(d, &sql).await?;
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
            rows: rs.rows.get(r).and_then(|x| x.get(2)).and_then(Value::as_i64).filter(|n| *n >= 0),
            comment: opt_text(&rs, r, 3),
            data_length: rs.rows.get(r).and_then(|x| x.get(4)).and_then(Value::as_i64),
            ..Default::default()
        })
        .collect())
}

pub(crate) async fn describe_table(d: &dyn Driver, schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
    let dl = Dialect::Postgres;
    let schema = schema.unwrap_or("public");
    let reg = format!("to_regclass({})", dl.string_literal(&dl.qualified(Some(schema), table)));
    let cols = q(d, &format!(
            "SELECT a.attname, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull, pg_get_expr(ad.adbin, ad.adrelid), a.attidentity <> '', \
             col_description(a.attrelid, a.attnum), EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = a.attrelid AND i.indisprimary AND a.attnum = ANY(i.indkey)) \
             FROM pg_attribute a LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum \
             WHERE a.attrelid = {reg} AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum"
        ))
        .await?;
    let columns = (0..cols.rows.len())
        .map(|r| ColumnInfo {
            name: text(&cols, r, 0),
            data_type: text(&cols, r, 1),
            nullable: truthy(&cols, r, 2),
            default: opt_text(&cols, r, 3),
            auto_increment: truthy(&cols, r, 4) || text(&cols, r, 3).starts_with("nextval("),
            comment: opt_text(&cols, r, 5),
            primary_key: truthy(&cols, r, 6),
        })
        .collect();
    let idx = q(d, &format!(
            "SELECT ic.relname, ix.indisunique, array_to_string(ARRAY(SELECT pg_get_indexdef(ix.indexrelid, k + 1, true) FROM generate_subscripts(ix.indkey, 1) k ORDER BY k), ',') \
             FROM pg_index ix JOIN pg_class ic ON ic.oid = ix.indexrelid WHERE ix.indrelid = {reg} AND NOT ix.indisprimary ORDER BY 1"
        ))
        .await?;
    let indexes = (0..idx.rows.len()).map(|r| IndexInfo { name: text(&idx, r, 0), unique: truthy(&idx, r, 1), columns: split_list(&text(&idx, r, 2)) }).collect();
    let fk = q(d, &format!(
            "SELECT c.conname, \
             (SELECT string_agg(a.attname, ',' ORDER BY k.n) FROM unnest(c.conkey) WITH ORDINALITY k(attnum, n) JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum), \
             (SELECT relname FROM pg_class WHERE oid = c.confrelid), \
             (SELECT string_agg(a.attname, ',' ORDER BY k.n) FROM unnest(c.confkey) WITH ORDINALITY k(attnum, n) JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum), \
             c.confdeltype, c.confupdtype FROM pg_constraint c WHERE c.contype = 'f' AND c.conrelid = {reg} ORDER BY 1"
        ))
        .await?;
    let foreign_keys = (0..fk.rows.len())
        .map(|r| ForeignKeyInfo {
            name: text(&fk, r, 0),
            columns: split_list(&text(&fk, r, 1)),
            ref_table: text(&fk, r, 2),
            ref_columns: split_list(&text(&fk, r, 3)),
            on_delete: fk_action(&text(&fk, r, 4)),
            on_update: fk_action(&text(&fk, r, 5)),
        })
        .collect();
    Ok(TableInfo { schema: Some(schema.to_string()), name: table.to_string(), columns, indexes, foreign_keys })
}
