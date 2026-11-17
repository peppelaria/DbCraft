//! Helpers shared by the `sqlx` drivers.

use std::time::Instant;

use db_core::{ColumnMeta, DbError, DbResult, ResultSet, RowChange, Value};
use futures_util::TryStreamExt;
use sqlx::{AssertSqlSafe, Column, Database, Either, Executor, Pool, Row, TypeInfo};

/// Rows kept per result set; beyond that the result is truncated (the UI says so).
pub const MAX_ROWS: usize = 500_000;

pub fn err(e: impl std::fmt::Display) -> DbError {
    DbError::Query(e.to_string())
}

/// Runs a script and returns one result per statement.
pub async fn run_many<DB, D, A>(pool: &Pool<DB>, sql: &str, decode: D, affected: A) -> DbResult<Vec<ResultSet>>
where
    DB: Database,
    for<'c> &'c Pool<DB>: Executor<'c, Database = DB>,
    D: Fn(&DB::Row, usize) -> Value,
    A: Fn(&DB::QueryResult) -> u64,
{
    let start = Instant::now();
    let mut out = Vec::new();
    let mut cur: Option<ResultSet> = None;
    let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.to_string())).fetch_many(pool);
    while let Some(item) = stream.try_next().await.map_err(err)? {
        match item {
            Either::Right(row) => {
                let rs = cur.get_or_insert_with(|| ResultSet {
                    columns: row.columns().iter().map(|c| ColumnMeta { name: c.name().to_string(), type_name: c.type_info().name().to_string() }).collect(),
                    sql: sql.to_string(),
                    ..Default::default()
                });
                if rs.rows.len() < MAX_ROWS {
                    rs.rows.push((0..row.len()).map(|i| decode(&row, i)).collect());
                }
            }
            Either::Left(done) => {
                let mut rs = cur.take().unwrap_or_else(|| ResultSet { affected: Some(affected(&done)), sql: sql.to_string(), ..Default::default() });
                rs.elapsed = start.elapsed();
                out.push(rs);
            }
        }
    }
    if let Some(mut rs) = cur.take() {
        rs.elapsed = start.elapsed();
        out.push(rs);
    }
    if out.is_empty() {
        out.push(ResultSet { affected: Some(0), elapsed: start.elapsed(), sql: sql.to_string(), ..Default::default() });
    }
    Ok(out)
}

/// Applies the statements in one transaction.
pub async fn apply_in_tx<DB, A>(pool: &Pool<DB>, statements: Vec<String>, affected: A) -> DbResult<u64>
where
    DB: Database,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    A: Fn(&DB::QueryResult) -> u64,
{
    let mut tx = pool.begin().await.map_err(err)?;
    let mut total = 0;
    for s in statements {
        let r = sqlx::raw_sql(AssertSqlSafe(s)).execute(&mut *tx).await.map_err(err)?;
        total += affected(&r);
    }
    tx.commit().await.map_err(err)?;
    Ok(total)
}

pub fn change_statements(d: db_core::Dialect, schema: Option<&str>, table: &str, changes: &[RowChange]) -> Vec<String> {
    changes.iter().map(|c| d.change_sql(schema, table, c)).collect()
}

/// Text of a cell of an introspection result.
pub fn text(rs: &ResultSet, row: usize, col: usize) -> String {
    match rs.rows.get(row).and_then(|r| r.get(col)) {
        Some(Value::Null) | None => String::new(),
        Some(Value::Bytes(b)) => String::from_utf8_lossy(b).into_owned(),
        Some(v) => v.to_string(),
    }
}

pub fn opt_text(rs: &ResultSet, row: usize, col: usize) -> Option<String> {
    match rs.rows.get(row).and_then(|r| r.get(col)) {
        Some(Value::Null) | None => None,
        Some(Value::Bytes(b)) => Some(String::from_utf8_lossy(b).into_owned()),
        Some(v) => Some(v.to_string()),
    }
}

pub fn truthy(rs: &ResultSet, row: usize, col: usize) -> bool {
    match rs.rows.get(row).and_then(|r| r.get(col)) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Int(i)) => *i != 0,
        Some(Value::Text(t)) => matches!(t.to_ascii_lowercase().as_str(), "t" | "true" | "1" | "yes" | "y"),
        _ => false,
    }
}

pub fn split_list(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

pub fn first(v: Vec<ResultSet>) -> ResultSet {
    v.into_iter().next().unwrap_or_default()
}
