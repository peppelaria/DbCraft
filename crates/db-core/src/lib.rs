//! Core types shared by DbCraft drivers and UI: values, result sets, schema metadata,
//! connection configuration, the `Driver` trait and per-dialect SQL generation.

pub mod config;
pub mod dialect;
pub mod format;
pub mod split;
pub mod schema;
pub mod value;

use std::time::Duration;

use async_trait::async_trait;

pub use config::{ConnectionConfig, DbKind, HttpTunnelConfig, SshAuth, SshConfig, TlsConfig, TlsVerify};
pub use dialect::Dialect;
pub use schema::{ForeignKeyDef, IndexDef, ColumnDef, ColumnInfo, ForeignKeyInfo, IndexInfo, ObjectKind, SchemaObject, TableDesign, TableInfo};
pub use value::Value;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("connessione: {0}")]
    Connection(String),
    #[error("{0}")]
    Query(String),
    #[error("non supportato: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Other(String),
}

pub type DbResult<T> = Result<T, DbError>;

#[derive(Clone, Debug, Default)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct ResultSet {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Value>>,
    /// Rows affected by a statement that returns no rows.
    pub affected: Option<u64>,
    pub elapsed: Duration,
    pub sql: String,
}

/// Where-clause / ordering applied to a table browse.
#[derive(Clone, Debug, Default)]
pub struct PageRequest {
    pub schema: Option<String>,
    pub table: String,
    /// Raw SQL condition (without `WHERE`).
    pub filter: Option<String>,
    /// (column, descending), in priority order.
    pub order: Vec<(String, bool)>,
    pub offset: u64,
    pub limit: u64,
}

/// One pending edit on a browsed table, keyed by primary-key values (or all original values when
/// the table has no key).
#[derive(Clone, Debug)]
pub enum RowChange {
    Update { key: Vec<(String, Value)>, set: Vec<(String, Value)> },
    Insert { values: Vec<(String, Value)> },
    Delete { key: Vec<(String, Value)> },
}

#[async_trait]
pub trait Driver: Send + Sync {
    fn dialect(&self) -> Dialect;
    async fn list_databases(&self) -> DbResult<Vec<String>>;
    /// Schemas within the current database (empty for engines without schemas).
    async fn list_schemas(&self) -> DbResult<Vec<String>>;
    async fn list_objects(&self, schema: Option<&str>) -> DbResult<Vec<SchemaObject>>;
    async fn describe_table(&self, schema: Option<&str>, table: &str) -> DbResult<TableInfo>;
    /// Runs one or more statements; returns one result per statement.
    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>>;
    async fn fetch_page(&self, req: &PageRequest) -> DbResult<ResultSet> {
        let d = self.dialect();
        let sql = d.select_page(req);
        let mut rs = self.execute(&sql).await?;
        rs.pop().ok_or_else(|| DbError::Query("nessun risultato".into()))
    }
    async fn count_rows(&self, schema: Option<&str>, table: &str, filter: Option<&str>) -> DbResult<u64> {
        let d = self.dialect();
        let mut sql = format!("SELECT COUNT(*) FROM {}", d.qualified(schema, table));
        if let Some(f) = filter.filter(|f| !f.trim().is_empty()) {
            sql.push_str(" WHERE ");
            sql.push_str(f);
        }
        let rs = self.execute(&sql).await?;
        Ok(rs.first().and_then(|r| r.rows.first()).and_then(|r| r.first()).and_then(Value::as_i64).unwrap_or(0).max(0) as u64)
    }
    /// The execution plan of one statement (`EXPLAIN`, `EXPLAIN QUERY PLAN` on SQLite).
    async fn explain(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        let prefix = if self.dialect() == Dialect::Sqlite { "EXPLAIN QUERY PLAN" } else { "EXPLAIN" };
        self.execute(&format!("{prefix} {}", sql.trim().trim_end_matches(';'))).await
    }
    /// Applies changes atomically.
    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64>;
    async fn close(&self) {}
}
