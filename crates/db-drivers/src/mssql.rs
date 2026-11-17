use std::time::Instant;

use async_trait::async_trait;
use db_core::*;
use futures_util::TryStreamExt;
use tiberius::{AuthMethod, Client, ColumnData, Config, FromSql, QueryItem};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::common::{MAX_ROWS, first, opt_text, split_list, text, truthy};

pub struct MssqlDriver {
    client: Mutex<Client<Compat<TcpStream>>>,
}

fn conn_err(e: impl std::fmt::Display) -> DbError {
    DbError::Connection(e.to_string())
}

fn qerr(e: impl std::fmt::Display) -> DbError {
    DbError::Query(e.to_string())
}

fn decode(d: ColumnData<'static>) -> Value {
    fn txt<T: std::fmt::Display>(v: Option<T>) -> Value {
        v.map_or(Value::Null, |x| Value::Text(x.to_string()))
    }
    match &d {
        ColumnData::U8(v) => v.map_or(Value::Null, |x| Value::Int(x.into())),
        ColumnData::I16(v) => v.map_or(Value::Null, |x| Value::Int(x.into())),
        ColumnData::I32(v) => v.map_or(Value::Null, |x| Value::Int(x.into())),
        ColumnData::I64(v) => v.map_or(Value::Null, Value::Int),
        ColumnData::F32(v) => v.map_or(Value::Null, |x| Value::Float(x.into())),
        ColumnData::F64(v) => v.map_or(Value::Null, Value::Float),
        ColumnData::Bit(v) => v.map_or(Value::Null, Value::Bool),
        ColumnData::String(v) => v.as_ref().map_or(Value::Null, |s| Value::Text(s.to_string())),
        ColumnData::Guid(v) => txt(*v),
        ColumnData::Binary(v) => v.as_ref().map_or(Value::Null, |b| Value::Bytes(b.to_vec())),
        ColumnData::Numeric(v) => txt(*v),
        ColumnData::Xml(v) => v.as_ref().map_or(Value::Null, |x| Value::Text(x.to_string())),
        ColumnData::DateTime(_) | ColumnData::SmallDateTime(_) | ColumnData::DateTime2(_) => txt(chrono::NaiveDateTime::from_sql(&d).ok().flatten()),
        ColumnData::Date(_) => txt(chrono::NaiveDate::from_sql(&d).ok().flatten()),
        ColumnData::Time(_) => txt(chrono::NaiveTime::from_sql(&d).ok().flatten()),
        ColumnData::DateTimeOffset(_) => txt(chrono::DateTime::<chrono::FixedOffset>::from_sql(&d).ok().flatten()),
    }
}

impl MssqlDriver {
    pub async fn connect(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Self> {
        let mut config = Config::new();
        config.host(if cfg.host.is_empty() { "localhost" } else { &cfg.host });
        config.port(if cfg.port == 0 { 1433 } else { cfg.port });
        config.authentication(AuthMethod::sql_server(&cfg.user, &cfg.password));
        config.application_name("DbCraft");
        let db = database.filter(|d| !d.is_empty()).unwrap_or(&cfg.database);
        if !db.is_empty() {
            config.database(db);
        }
        if cfg.ssl {
            cfg.tls.check_files().map_err(DbError::Connection)?;
            config.encryption(tiberius::EncryptionLevel::Required);
            // `trust_cert` and `trust_cert_ca` are mutually exclusive (tiberius panics on both).
            match (cfg.tls.verify, cfg.tls.ca_cert.trim()) {
                (TlsVerify::None, _) => config.trust_cert(),
                (_, "") => {} // the system's root certificates
                (_, ca) => config.trust_cert_ca(ca),
            }
            if !cfg.tls.client_cert.trim().is_empty() {
                config.client_certificate(cfg.tls.client_cert.trim(), cfg.tls.client_key.trim());
            }
        } else {
            // Local/dev servers usually present a self-signed certificate.
            config.trust_cert();
        }
        let tcp = TcpStream::connect(config.get_addr()).await.map_err(conn_err)?;
        tcp.set_nodelay(true).map_err(conn_err)?;
        let client = Client::connect(config, tcp.compat_write()).await.map_err(conn_err)?;
        Ok(MssqlDriver { client: Mutex::new(client) })
    }

    async fn q(&self, sql: &str) -> DbResult<ResultSet> {
        Ok(first(self.execute(sql).await?))
    }

    fn obj(schema: Option<&str>, table: &str) -> String {
        let d = Dialect::SqlServer;
        format!("OBJECT_ID({})", d.string_literal(&d.qualified(Some(schema.unwrap_or("dbo")), table)))
    }
}

#[async_trait]
impl Driver for MssqlDriver {
    fn dialect(&self) -> Dialect {
        Dialect::SqlServer
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let rs = self.q("SELECT name FROM sys.databases ORDER BY name").await?;
        Ok((0..rs.rows.len()).map(|r| text(&rs, r, 0)).collect())
    }

    async fn list_schemas(&self) -> DbResult<Vec<String>> {
        let rs = self
            .q("SELECT name FROM sys.schemas WHERE principal_id < 16384 AND name NOT IN ('sys','INFORMATION_SCHEMA','guest') AND name NOT LIKE 'db[_]%' ORDER BY name")
            .await?;
        Ok((0..rs.rows.len()).map(|r| text(&rs, r, 0)).collect())
    }

    async fn list_objects(&self, schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
        let s = Dialect::SqlServer.string_literal(schema.unwrap_or("dbo"));
        let rs = self
            .q(&format!(
                "SELECT o.name, RTRIM(o.type), (SELECT SUM(p.rows) FROM sys.partitions p WHERE p.object_id = o.object_id AND p.index_id IN (0,1)) \
                 FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id \
                 WHERE s.name = {s} AND o.type IN ('U','V','P','FN','IF','TF','TR') AND o.is_ms_shipped = 0 ORDER BY o.name"
            ))
            .await?;
        Ok((0..rs.rows.len())
            .map(|r| SchemaObject {
                name: text(&rs, r, 0),
                kind: match text(&rs, r, 1).as_str() {
                    "V" => ObjectKind::View,
                    "P" => ObjectKind::Procedure,
                    "FN" | "IF" | "TF" => ObjectKind::Function,
                    "TR" => ObjectKind::Trigger,
                    _ => ObjectKind::Table,
                },
                rows: rs.rows.get(r).and_then(|x| x.get(2)).and_then(Value::as_i64),
                comment: None,
            })
            .collect())
    }

    async fn describe_table(&self, schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
        let obj = Self::obj(schema, table);
        let cols = self
            .q(&format!(
                "SELECT c.name, TYPE_NAME(c.user_type_id) + CASE \
                   WHEN TYPE_NAME(c.user_type_id) IN ('varchar','char','varbinary','binary') THEN '(' + CASE WHEN c.max_length = -1 THEN 'max' ELSE CAST(c.max_length AS varchar(10)) END + ')' \
                   WHEN TYPE_NAME(c.user_type_id) IN ('nvarchar','nchar') THEN '(' + CASE WHEN c.max_length = -1 THEN 'max' ELSE CAST(c.max_length / 2 AS varchar(10)) END + ')' \
                   WHEN TYPE_NAME(c.user_type_id) IN ('decimal','numeric') THEN '(' + CAST(c.precision AS varchar(5)) + ',' + CAST(c.scale AS varchar(5)) + ')' ELSE '' END, \
                 c.is_nullable, OBJECT_DEFINITION(c.default_object_id), c.is_identity, \
                 CASE WHEN EXISTS (SELECT 1 FROM sys.index_columns ic JOIN sys.indexes i ON i.object_id = ic.object_id AND i.index_id = ic.index_id \
                   WHERE i.is_primary_key = 1 AND ic.object_id = c.object_id AND ic.column_id = c.column_id) THEN 1 ELSE 0 END \
                 FROM sys.columns c WHERE c.object_id = {obj} ORDER BY c.column_id"
            ))
            .await?;
        let columns = (0..cols.rows.len())
            .map(|r| ColumnInfo {
                name: text(&cols, r, 0),
                data_type: text(&cols, r, 1),
                nullable: truthy(&cols, r, 2),
                default: opt_text(&cols, r, 3),
                auto_increment: truthy(&cols, r, 4),
                primary_key: truthy(&cols, r, 5),
                comment: None,
            })
            .collect();
        let idx = self
            .q(&format!(
                "SELECT i.name, i.is_unique, STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.indexes i \
                 JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id \
                 JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
                 WHERE i.object_id = {obj} AND i.is_primary_key = 0 AND i.name IS NOT NULL AND ic.is_included_column = 0 GROUP BY i.name, i.is_unique"
            ))
            .await?;
        let indexes = (0..idx.rows.len()).map(|r| IndexInfo { name: text(&idx, r, 0), unique: truthy(&idx, r, 1), columns: split_list(&text(&idx, r, 2)) }).collect();
        let fk = self
            .q(&format!(
                "SELECT fk.name, STRING_AGG(pc.name, ','), OBJECT_NAME(fk.referenced_object_id), STRING_AGG(rc.name, ','), \
                 fk.delete_referential_action_desc, fk.update_referential_action_desc FROM sys.foreign_keys fk \
                 JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id \
                 JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id AND pc.column_id = fkc.parent_column_id \
                 JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id \
                 WHERE fk.parent_object_id = {obj} GROUP BY fk.name, fk.referenced_object_id, fk.delete_referential_action_desc, fk.update_referential_action_desc"
            ))
            .await?;
        let action = |s: String| Some(s.replace('_', " ")).filter(|a| a != "NO ACTION");
        let foreign_keys = (0..fk.rows.len())
            .map(|r| ForeignKeyInfo {
                name: text(&fk, r, 0),
                columns: split_list(&text(&fk, r, 1)),
                ref_table: text(&fk, r, 2),
                ref_columns: split_list(&text(&fk, r, 3)),
                on_delete: action(text(&fk, r, 4)),
                on_update: action(text(&fk, r, 5)),
            })
            .collect();
        Ok(TableInfo { schema: Some(schema.unwrap_or("dbo").to_string()), name: table.to_string(), columns, indexes, foreign_keys })
    }

    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        let start = Instant::now();
        let mut client = self.client.lock().await;
        let mut stream = client.simple_query(sql).await.map_err(qerr)?;
        let mut out: Vec<ResultSet> = Vec::new();
        while let Some(item) = stream.try_next().await.map_err(qerr)? {
            match item {
                QueryItem::Metadata(meta) => out.push(ResultSet {
                    columns: meta.columns().iter().map(|c| ColumnMeta { name: c.name().to_string(), type_name: type_name(c.column_type()).to_string() }).collect(),
                    sql: sql.to_string(),
                    ..Default::default()
                }),
                QueryItem::Row(row) => {
                    if let Some(rs) = out.last_mut()
                        && rs.rows.len() < MAX_ROWS
                    {
                        rs.rows.push(row.into_iter().map(decode).collect());
                    }
                }
            }
        }
        let elapsed = start.elapsed();
        if out.is_empty() {
            out.push(ResultSet { affected: None, sql: sql.to_string(), ..Default::default() });
        }
        for rs in &mut out {
            rs.elapsed = elapsed;
        }
        Ok(out)
    }

    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64> {
        let d = Dialect::SqlServer;
        let mut batch = String::from("SET XACT_ABORT ON; BEGIN TRANSACTION;\n");
        for c in changes {
            batch.push_str(&d.change_sql(schema, table, c));
            batch.push_str(";\n");
        }
        batch.push_str("COMMIT;");
        let mut client = self.client.lock().await;
        let res = client.execute(batch, &[]).await.map_err(qerr)?;
        Ok(res.rows_affected().iter().sum())
    }
}

/// SQL Server type name for a TDS column type (the protocol has nullable variants such as
/// `Intn` that the user never wrote).
fn type_name(t: tiberius::ColumnType) -> &'static str {
    use tiberius::ColumnType as C;
    match t {
        C::Null => "",
        C::Bit | C::Bitn => "bit",
        C::Int1 => "tinyint",
        C::Int2 => "smallint",
        C::Int4 | C::Intn => "int",
        C::Int8 => "bigint",
        C::Float4 => "real",
        C::Float8 | C::Floatn => "float",
        C::Money => "money",
        C::Money4 => "smallmoney",
        C::Datetime | C::Datetimen => "datetime",
        C::Datetime4 => "smalldatetime",
        C::Datetime2 => "datetime2",
        C::DatetimeOffsetn => "datetimeoffset",
        C::Daten => "date",
        C::Timen => "time",
        C::Decimaln => "decimal",
        C::Numericn => "numeric",
        C::Guid => "uniqueidentifier",
        C::BigVarBin => "varbinary",
        C::BigBinary => "binary",
        C::Image => "image",
        C::BigVarChar => "varchar",
        C::BigChar => "char",
        C::Text => "text",
        C::NVarchar => "nvarchar",
        C::NChar => "nchar",
        C::NText => "ntext",
        C::Xml => "xml",
        C::Udt => "udt",
        C::SSVariant => "sql_variant",
    }
}
