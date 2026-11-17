//! Export of result sets (CSV, JSON, SQL INSERT, Excel) and CSV reading for import.

use std::io::Write;
use std::path::Path;

use db_core::{Dialect, ResultSet, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
    Sql,
    Xlsx,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Csv, Format::Json, Format::Sql, Format::Xlsx];
    pub fn label(self) -> &'static str {
        match self {
            Format::Csv => "CSV (*.csv)",
            Format::Json => "JSON (*.json)",
            Format::Sql => "Script SQL INSERT (*.sql)",
            Format::Xlsx => "Excel (*.xlsx)",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
            Format::Sql => "sql",
            Format::Xlsx => "xlsx",
        }
    }
}

fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub fn export(rs: &ResultSet, path: &Path, fmt: Format, dialect: Dialect, table: &str, header: bool) -> Result<usize, String> {
    let e = |x: &dyn std::fmt::Display| x.to_string();
    match fmt {
        Format::Csv => {
            let mut w = csv::Writer::from_path(path).map_err(|x| e(&x))?;
            if header {
                w.write_record(rs.columns.iter().map(|c| c.name.as_str())).map_err(|x| e(&x))?;
            }
            for r in &rs.rows {
                w.write_record(r.iter().map(cell_text)).map_err(|x| e(&x))?;
            }
            w.flush().map_err(|x| e(&x))?;
        }
        Format::Json => {
            let rows: Vec<serde_json::Value> = rs
                .rows
                .iter()
                .map(|r| serde_json::Value::Object(rs.columns.iter().zip(r).map(|(c, v)| (c.name.clone(), v.to_json())).collect()))
                .collect();
            let text = serde_json::to_string_pretty(&rows).map_err(|x| e(&x))?;
            std::fs::write(path, text).map_err(|x| e(&x))?;
        }
        Format::Sql => {
            let mut f = std::io::BufWriter::new(std::fs::File::create(path).map_err(|x| e(&x))?);
            let cols: Vec<String> = rs.columns.iter().map(|c| c.name.clone()).collect();
            for r in &rs.rows {
                writeln!(f, "{};", dialect.insert_sql(None, table, &cols, r)).map_err(|x| e(&x))?;
            }
            f.flush().map_err(|x| e(&x))?;
        }
        Format::Xlsx => {
            let mut wb = rust_xlsxwriter::Workbook::new();
            let ws = wb.add_worksheet();
            let bold = rust_xlsxwriter::Format::new().set_bold();
            let mut row0 = 0u32;
            if header {
                for (i, c) in rs.columns.iter().enumerate() {
                    ws.write_string_with_format(0, i as u16, &c.name, &bold).map_err(|x| e(&x))?;
                }
                row0 = 1;
            }
            for (ri, r) in rs.rows.iter().enumerate().take(1_048_000) {
                let rr = row0 + ri as u32;
                for (ci, v) in r.iter().enumerate().take(16_000) {
                    let c = ci as u16;
                    match v {
                        Value::Null => continue,
                        Value::Int(i) => ws.write_number(rr, c, *i as f64),
                        Value::Float(x) => ws.write_number(rr, c, *x),
                        Value::Bool(b) => ws.write_boolean(rr, c, *b),
                        other => ws.write_string(rr, c, cell_text(other)),
                    }
                    .map_err(|x| e(&x))?;
                }
            }
            wb.save(path).map_err(|x| e(&x))?;
        }
    }
    Ok(rs.rows.len())
}

pub struct CsvData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn read_csv(path: &Path, delimiter: u8, has_header: bool) -> Result<CsvData, String> {
    let mut r = csv::ReaderBuilder::new().delimiter(delimiter).has_headers(false).flexible(true).from_path(path).map_err(|e| e.to_string())?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in r.records() {
        let rec = rec.map_err(|e| e.to_string())?;
        rows.push(rec.iter().map(str::to_string).collect());
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let headers = if has_header && !rows.is_empty() { rows.remove(0) } else { (1..=width).map(|i| format!("campo{i}")).collect() };
    Ok(CsvData { headers, rows })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use db_core::ColumnMeta;

    #[test]
    fn csv_roundtrip() {
        let rs = ResultSet {
            columns: vec![ColumnMeta { name: "a".into(), type_name: String::new() }, ColumnMeta { name: "b".into(), type_name: String::new() }],
            rows: vec![vec![Value::Int(1), Value::Text("x,y".into())], vec![Value::Null, Value::Text("z".into())]],
            ..Default::default()
        };
        let dir = std::env::temp_dir().join(format!("dbcraft-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.csv");
        export(&rs, &p, Format::Csv, Dialect::Sqlite, "t", true).unwrap();
        let d = read_csv(&p, b',', true).unwrap();
        assert_eq!(d.headers, vec!["a", "b"]);
        assert_eq!(d.rows, vec![vec!["1", "x,y"], vec!["", "z"]]);
        let p = dir.join("t.sql");
        export(&rs, &p, Format::Sql, Dialect::MySql, "t", true).unwrap();
        let sql = std::fs::read_to_string(&p).unwrap();
        assert!(sql.starts_with("INSERT INTO `t` (`a`, `b`) VALUES (1, 'x,y');"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
