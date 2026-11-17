//! HTTP tunnel driver: talks to `tunnel/dbcraft-tunnel.php` on a web server, which runs the SQL
//! with PDO (MySQL/MariaDB, PostgreSQL). Introspection reuses the MySQL/PostgreSQL queries.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use db_core::split::split_statements;
use db_core::*;
use serde_json::{Value as Json, json};

pub struct HttpDriver {
    client: reqwest::Client,
    cfg: HttpTunnelConfig,
    dialect: Dialect,
    db: Json,
}

fn conn_err(e: impl std::fmt::Display) -> DbError {
    DbError::Connection(format!("Tunnel HTTP: {e}"))
}

fn read(path: &str) -> DbResult<Vec<u8>> {
    std::fs::read(path.trim()).map_err(|e| conn_err(format!("{}: {e}", path.trim())))
}

fn to_value(v: &Json) -> Value {
    match v {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n.as_i64().map(Value::Int).or_else(|| n.as_f64().map(Value::Float)).unwrap_or_else(|| Value::Text(n.to_string())),
        Json::String(s) => Value::Text(s.clone()),
        Json::Object(o) => match o.get("$b64").and_then(Json::as_str) {
            Some(b) => decode_b64(b).map_or_else(|| Value::Text(b.to_string()), Value::Bytes),
            None => Value::Text(v.to_string()),
        },
        Json::Array(_) => Value::Text(v.to_string()),
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_b64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn decode_b64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|x| *x == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl HttpDriver {
    pub async fn connect(cfg: &ConnectionConfig, database: Option<&str>) -> DbResult<Self> {
        let t = &cfg.http;
        let (driver, dialect) = match cfg.kind {
            DbKind::MySql | DbKind::MariaDb => ("mysql", Dialect::MySql),
            DbKind::Postgres => ("pgsql", Dialect::Postgres),
            other => return Err(conn_err(format!("{} non è supportato dal tunnel HTTP (solo MySQL/MariaDB e PostgreSQL)", other.label()))),
        };
        if t.url.trim().is_empty() {
            return Err(conn_err("URL del tunnel mancante"));
        }
        let mut b = reqwest::Client::builder().timeout(Duration::from_secs(120)).connect_timeout(Duration::from_secs(20)).user_agent("DbCraft");
        if !t.ca_cert.trim().is_empty() {
            for c in reqwest::Certificate::from_pem_bundle(&read(&t.ca_cert)?).map_err(conn_err)? {
                b = b.add_root_certificate(c);
            }
        }
        if !t.client_cert.trim().is_empty() {
            let mut pem = read(&t.client_cert)?;
            if !t.client_key.trim().is_empty() {
                pem.push(b'\n');
                pem.extend(read(&t.client_key)?);
            }
            b = b.identity(reqwest::Identity::from_pem(&pem).map_err(conn_err)?);
        }
        if t.proxy && !t.proxy_host.trim().is_empty() {
            let port = if t.proxy_port == 0 { 8080 } else { t.proxy_port };
            let host = t.proxy_host.trim();
            let url = if host.contains("://") { format!("{host}:{port}") } else { format!("http://{host}:{port}") };
            let mut p = reqwest::Proxy::all(url).map_err(conn_err)?;
            if !t.proxy_user.is_empty() {
                p = p.basic_auth(&t.proxy_user, &t.proxy_password);
            }
            b = b.proxy(p);
        }
        let client = b.build().map_err(conn_err)?;
        let dbname = database.filter(|d| !d.is_empty()).unwrap_or(&cfg.database);
        let db = json!({
            "driver": driver,
            "host": if cfg.host.trim().is_empty() { "localhost" } else { cfg.host.trim() },
            "port": if cfg.port == 0 { cfg.kind.default_port() } else { cfg.port },
            "user": cfg.user,
            "password": cfg.password,
            "database": dbname,
        });
        let d = HttpDriver { client, cfg: t.clone(), dialect, db };
        let r = d.post("ping", &[]).await?;
        if r.get("ok").and_then(Json::as_bool) != Some(true) {
            return Err(conn_err(r.get("error").and_then(Json::as_str).unwrap_or("risposta non valida")));
        }
        Ok(d)
    }

    async fn post(&self, action: &str, statements: &[String]) -> DbResult<Json> {
        let stmts: Vec<String> = if self.cfg.base64 { statements.iter().map(|s| encode_b64(s.as_bytes())).collect() } else { statements.to_vec() };
        let body = json!({ "v": 1, "action": action, "db": self.db, "statements": stmts, "base64": self.cfg.base64, "max_rows": crate::common::MAX_ROWS });
        let mut req = self.client.post(self.cfg.url.trim()).json(&body);
        if self.cfg.basic_auth {
            req = req.basic_auth(&self.cfg.user, Some(&self.cfg.password));
        }
        if !self.cfg.token.is_empty() {
            req = req.header("X-DbCraft-Token", &self.cfg.token);
        }
        let resp = req.send().await.map_err(conn_err)?;
        let status = resp.status();
        let text = resp.text().await.map_err(conn_err)?;
        let v: Json = serde_json::from_str(&text).map_err(|_| {
            let snippet: String = text.chars().take(200).collect();
            conn_err(format!("risposta non JSON (HTTP {status}): {snippet}"))
        })?;
        if !status.is_success() {
            return Err(conn_err(format!("HTTP {status}: {}", v.get("error").and_then(Json::as_str).unwrap_or(""))));
        }
        Ok(v)
    }
}

fn result_set(r: &Json, sql: &str, elapsed: Duration) -> ResultSet {
    let columns: Vec<ColumnMeta> = r
        .get("columns")
        .and_then(Json::as_array)
        .map(|cs| {
            cs.iter()
                .map(|c| ColumnMeta { name: c.get("name").and_then(Json::as_str).unwrap_or("").to_string(), type_name: c.get("type").and_then(Json::as_str).unwrap_or("").to_string() })
                .collect()
        })
        .unwrap_or_default();
    let rows = r.get("rows").and_then(Json::as_array).map(|rows| rows.iter().map(|row| row.as_array().map(|cells| cells.iter().map(to_value).collect()).unwrap_or_default()).collect()).unwrap_or_default();
    let affected = if columns.is_empty() { r.get("affected").and_then(Json::as_u64) } else { None };
    ResultSet { columns, rows, affected, elapsed, sql: sql.to_string() }
}

#[async_trait]
impl Driver for HttpDriver {
    fn dialect(&self) -> Dialect {
        self.dialect
    }
    async fn list_databases(&self) -> DbResult<Vec<String>> {
        match self.dialect {
            Dialect::Postgres => crate::postgres::list_databases(self).await,
            _ => crate::mysql::list_databases(self).await,
        }
    }
    async fn list_schemas(&self) -> DbResult<Vec<String>> {
        match self.dialect {
            Dialect::Postgres => crate::postgres::list_schemas(self).await,
            _ => crate::mysql::list_schemas(self).await,
        }
    }
    async fn list_objects(&self, schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
        match self.dialect {
            Dialect::Postgres => crate::postgres::list_objects(self, schema).await,
            _ => crate::mysql::list_objects(self, schema).await,
        }
    }
    async fn describe_table(&self, schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
        match self.dialect {
            Dialect::Postgres => crate::postgres::describe_table(self, schema, table).await,
            _ => crate::mysql::describe_table(self, schema, table).await,
        }
    }
    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        let start = Instant::now();
        let statements = split_statements(sql, self.dialect);
        if statements.is_empty() {
            return Ok(vec![ResultSet { affected: Some(0), sql: sql.to_string(), ..Default::default() }]);
        }
        let v = self.post("query", &statements).await?;
        if v.get("ok").and_then(Json::as_bool) != Some(true) {
            return Err(DbError::Query(v.get("error").and_then(Json::as_str).unwrap_or("errore sconosciuto").to_string()));
        }
        let elapsed = start.elapsed();
        Ok(v.get("results").and_then(Json::as_array).map(|rs| rs.iter().map(|r| result_set(r, sql, elapsed)).collect()).unwrap_or_default())
    }
    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64> {
        let statements: Vec<String> = changes.iter().map(|c| self.dialect.change_sql(schema, table, c)).collect();
        let v = self.post("apply", &statements).await?;
        if v.get("ok").and_then(Json::as_bool) != Some(true) {
            return Err(DbError::Query(v.get("error").and_then(Json::as_str).unwrap_or("errore sconosciuto").to_string()));
        }
        Ok(v.get("affected").and_then(Json::as_u64).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        for s in ["", "a", "ab", "abc", "SELECT 'è';\n", "\u{0}\u{ff}"] {
            assert_eq!(decode_b64(&encode_b64(s.as_bytes())).as_deref(), Some(s.as_bytes()), "{s:?}");
        }
        assert_eq!(encode_b64(b"Man"), "TWFu");
        assert_eq!(encode_b64(b"Ma"), "TWE=");
    }

    #[test]
    fn json_values() {
        assert_eq!(to_value(&json!(null)), Value::Null);
        assert_eq!(to_value(&json!(5)), Value::Int(5));
        assert_eq!(to_value(&json!(1.5)), Value::Float(1.5));
        assert_eq!(to_value(&json!("x")), Value::Text("x".into()));
        assert_eq!(to_value(&json!({"$b64": "AQL/"})), Value::Bytes(vec![1, 2, 255]));
    }
}
