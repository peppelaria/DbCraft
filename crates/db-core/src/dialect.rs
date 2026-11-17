//! Per-dialect SQL generation: quoting, literals, paging, row changes and DDL.

use crate::config::DbKind;
use crate::schema::{ColumnDef, ForeignKeyDef, IndexDef, TableDesign};
use crate::value::Value;
use crate::{PageRequest, RowChange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Sqlite,
    Postgres,
    MySql,
    SqlServer,
}

impl From<DbKind> for Dialect {
    fn from(k: DbKind) -> Self {
        match k {
            DbKind::Sqlite => Dialect::Sqlite,
            DbKind::Postgres => Dialect::Postgres,
            DbKind::MySql | DbKind::MariaDb => Dialect::MySql,
            DbKind::SqlServer => Dialect::SqlServer,
        }
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

impl Dialect {
    /// Common data types offered by the table designer.
    pub fn types(self) -> &'static [&'static str] {
        match self {
            Dialect::Sqlite => &["INTEGER", "TEXT", "REAL", "NUMERIC", "BLOB", "BOOLEAN", "DATE", "DATETIME"],
            Dialect::Postgres => &[
                "integer", "bigint", "smallint", "serial", "bigserial", "numeric(10,2)", "real", "double precision", "boolean", "varchar(255)", "text", "char(1)",
                "date", "time", "timestamp", "timestamptz", "uuid", "json", "jsonb", "bytea",
            ],
            Dialect::MySql => &[
                "int", "bigint", "smallint", "tinyint", "decimal(10,2)", "float", "double", "varchar(255)", "char(1)", "text", "longtext", "date", "datetime",
                "timestamp", "time", "json", "blob", "longblob", "enum('a','b')",
            ],
            Dialect::SqlServer => &[
                "int", "bigint", "smallint", "tinyint", "bit", "decimal(10,2)", "float", "real", "nvarchar(255)", "varchar(255)", "nvarchar(max)", "char(1)", "date",
                "datetime2", "datetimeoffset", "time", "uniqueidentifier", "varbinary(max)",
            ],
        }
    }

    pub fn quote(self, ident: &str) -> String {
        match self {
            Dialect::MySql => format!("`{}`", ident.replace('`', "``")),
            Dialect::SqlServer => format!("[{}]", ident.replace(']', "]]")),
            Dialect::Sqlite | Dialect::Postgres => format!("\"{}\"", ident.replace('"', "\"\"")),
        }
    }

    pub fn qualified(self, schema: Option<&str>, name: &str) -> String {
        match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote(s), self.quote(name)),
            None => self.quote(name),
        }
    }

    pub fn string_literal(self, s: &str) -> String {
        let body = s.replace('\'', "''");
        match self {
            Dialect::MySql => format!("'{}'", body.replace('\\', "\\\\")),
            Dialect::SqlServer => format!("N'{body}'"),
            _ => format!("'{body}'"),
        }
    }

    pub fn literal(self, v: &Value) -> String {
        match v {
            Value::Null => "NULL".into(),
            Value::Bool(b) => match self {
                Dialect::Postgres => if *b { "TRUE" } else { "FALSE" }.into(),
                _ => if *b { "1" } else { "0" }.into(),
            },
            Value::Int(i) => i.to_string(),
            Value::Float(f) if f.is_finite() => f.to_string(),
            Value::Float(_) => "NULL".into(),
            Value::Text(t) => self.string_literal(t),
            Value::Bytes(b) => {
                let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
                match self {
                    Dialect::Postgres => format!("'\\x{h}'::bytea"),
                    Dialect::SqlServer => format!("0x{h}"),
                    _ => format!("X'{h}'"),
                }
            }
        }
    }

    pub fn select_page(self, r: &PageRequest) -> String {
        let mut sql = format!("SELECT * FROM {}", self.qualified(r.schema.as_deref(), &r.table));
        if let Some(f) = r.filter.as_deref().filter(|f| !f.trim().is_empty()) {
            sql.push_str(" WHERE ");
            sql.push_str(f);
        }
        match &r.order {
            Some((c, desc)) => {
                sql.push_str(&format!(" ORDER BY {}{}", self.quote(c), if *desc { " DESC" } else { "" }));
            }
            None if self == Dialect::SqlServer => sql.push_str(" ORDER BY (SELECT NULL)"),
            None => {}
        }
        match self {
            Dialect::SqlServer => sql.push_str(&format!(" OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", r.offset, r.limit)),
            _ => sql.push_str(&format!(" LIMIT {} OFFSET {}", r.limit, r.offset)),
        }
        sql
    }

    fn where_key(self, key: &[(String, Value)]) -> String {
        key.iter()
            .map(|(c, v)| if v.is_null() { format!("{} IS NULL", self.quote(c)) } else { format!("{} = {}", self.quote(c), self.literal(v)) })
            .collect::<Vec<_>>()
            .join(" AND ")
    }

    /// SQL for one row change. Updates/deletes on tables without a key are limited to one row
    /// where the dialect allows it.
    pub fn change_sql(self, schema: Option<&str>, table: &str, change: &RowChange) -> String {
        let t = self.qualified(schema, table);
        match change {
            RowChange::Update { key, set } => {
                let s = set.iter().map(|(c, v)| format!("{} = {}", self.quote(c), self.literal(v))).collect::<Vec<_>>().join(", ");
                format!("UPDATE {t} SET {s} WHERE {}", self.where_key(key))
            }
            RowChange::Insert { values } => self.insert_sql(schema, table, &values.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(), &values.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>()),
            RowChange::Delete { key } => format!("DELETE FROM {t} WHERE {}", self.where_key(key)),
        }
    }

    pub fn insert_sql(self, schema: Option<&str>, table: &str, cols: &[String], values: &[Value]) -> String {
        if cols.is_empty() {
            let t = self.qualified(schema, table);
            return if self == Dialect::MySql { format!("INSERT INTO {t} () VALUES ()") } else { format!("INSERT INTO {t} DEFAULT VALUES") };
        }
        let c = cols.iter().map(|c| self.quote(c)).collect::<Vec<_>>().join(", ");
        let v = values.iter().map(|v| self.literal(v)).collect::<Vec<_>>().join(", ");
        format!("INSERT INTO {} ({c}) VALUES ({v})", self.qualified(schema, table))
    }

    fn column_sql(self, c: &ColumnDef, inline_pk: bool) -> String {
        let mut ty = c.data_type.trim().to_string();
        if ty.is_empty() {
            ty = self.types().first().copied().unwrap_or("TEXT").to_string();
        }
        let mut s = format!("{} {ty}", self.quote(&c.name));
        if inline_pk {
            s.push_str(" PRIMARY KEY");
            if c.auto_increment {
                s.push_str(" AUTOINCREMENT");
            }
        }
        if c.auto_increment && !inline_pk {
            match self {
                Dialect::MySql => s.push_str(" AUTO_INCREMENT"),
                Dialect::Postgres => s.push_str(" GENERATED BY DEFAULT AS IDENTITY"),
                Dialect::SqlServer => s.push_str(" IDENTITY(1,1)"),
                Dialect::Sqlite => {}
            }
        }
        if c.not_null || c.primary_key {
            s.push_str(" NOT NULL");
        } else if self != Dialect::Sqlite {
            s.push_str(" NULL");
        }
        if !c.default.trim().is_empty() {
            s.push_str(" DEFAULT ");
            s.push_str(c.default.trim());
        }
        if self == Dialect::MySql && !c.comment.is_empty() {
            s.push_str(" COMMENT ");
            s.push_str(&self.string_literal(&c.comment));
        }
        s
    }

    fn fk_sql(self, f: &ForeignKeyDef) -> String {
        let cols = |s: &str| split_list(s).iter().map(|c| self.quote(c)).collect::<Vec<_>>().join(", ");
        let mut s = String::new();
        if !f.name.trim().is_empty() {
            s.push_str(&format!("CONSTRAINT {} ", self.quote(f.name.trim())));
        }
        s.push_str(&format!("FOREIGN KEY ({}) REFERENCES {} ({})", cols(&f.columns), self.quote(&f.ref_table), cols(&f.ref_columns)));
        if !f.on_delete.trim().is_empty() {
            s.push_str(&format!(" ON DELETE {}", f.on_delete.trim()));
        }
        if !f.on_update.trim().is_empty() {
            s.push_str(&format!(" ON UPDATE {}", f.on_update.trim()));
        }
        s
    }

    fn index_sql(self, d: &TableDesign, i: &IndexDef) -> String {
        let cols = split_list(&i.columns).iter().map(|c| self.quote(c)).collect::<Vec<_>>().join(", ");
        let name = match self {
            Dialect::Postgres | Dialect::Sqlite => self.qualified(None, &i.name),
            _ => self.quote(&i.name),
        };
        format!("CREATE {}INDEX {name} ON {} ({cols})", if i.unique { "UNIQUE " } else { "" }, self.qualified(d.schema.as_deref(), &d.name))
    }

    fn drop_index_sql(self, d: &TableDesign, name: &str) -> String {
        match self {
            Dialect::MySql | Dialect::SqlServer => format!("DROP INDEX {} ON {}", self.quote(name), self.qualified(d.schema.as_deref(), &d.name)),
            Dialect::Postgres => format!("DROP INDEX {}", self.qualified(d.schema.as_deref(), name)),
            Dialect::Sqlite => format!("DROP INDEX {}", self.quote(name)),
        }
    }

    pub fn create_table(self, d: &TableDesign) -> Vec<String> {
        let pk: Vec<&ColumnDef> = d.columns.iter().filter(|c| c.primary_key).collect();
        let sqlite_inline = self == Dialect::Sqlite && pk.len() == 1 && pk.iter().any(|c| c.auto_increment);
        let mut parts: Vec<String> = d.columns.iter().map(|c| self.column_sql(c, sqlite_inline && c.primary_key)).collect();
        if !pk.is_empty() && !sqlite_inline {
            parts.push(format!("PRIMARY KEY ({})", pk.iter().map(|c| self.quote(&c.name)).collect::<Vec<_>>().join(", ")));
        }
        for f in &d.foreign_keys {
            parts.push(self.fk_sql(f));
        }
        let mut out = vec![format!("CREATE TABLE {} (\n  {}\n)", self.qualified(d.schema.as_deref(), &d.name), parts.join(",\n  "))];
        for i in &d.indexes {
            out.push(self.index_sql(d, i));
        }
        if self == Dialect::Postgres {
            for c in d.columns.iter().filter(|c| !c.comment.is_empty()) {
                out.push(format!("COMMENT ON COLUMN {}.{} IS {}", self.qualified(d.schema.as_deref(), &d.name), self.quote(&c.name), self.string_literal(&c.comment)));
            }
        }
        out
    }

    /// Statements turning `old` into `new`.
    pub fn alter_table(self, old: &TableDesign, new: &TableDesign) -> Vec<String> {
        if old == new {
            return Vec::new();
        }
        if self == Dialect::Sqlite {
            return self.sqlite_rebuild(old, new);
        }
        let t = self.qualified(new.schema.as_deref(), &old.name);
        let mut out = Vec::new();
        // Dropped columns.
        for c in &old.columns {
            if !new.columns.iter().any(|n| n.original.as_deref() == Some(c.name.as_str())) {
                out.push(format!("ALTER TABLE {t} DROP COLUMN {}", self.quote(&c.name)));
            }
        }
        // Dropped / changed foreign keys and indexes.
        for f in &old.foreign_keys {
            if !new.foreign_keys.contains(f) && !f.name.is_empty() {
                out.push(match self {
                    Dialect::MySql => format!("ALTER TABLE {t} DROP FOREIGN KEY {}", self.quote(&f.name)),
                    _ => format!("ALTER TABLE {t} DROP CONSTRAINT {}", self.quote(&f.name)),
                });
            }
        }
        for i in &old.indexes {
            if !new.indexes.contains(i) {
                out.push(self.drop_index_sql(old, &i.name));
            }
        }
        for c in &new.columns {
            match c.original.as_deref().and_then(|o| old.columns.iter().find(|x| x.name == o)) {
                None => out.push(match self {
                    Dialect::SqlServer => format!("ALTER TABLE {t} ADD {}", self.column_sql(c, false)),
                    _ => format!("ALTER TABLE {t} ADD COLUMN {}", self.column_sql(c, false)),
                }),
                Some(o) => {
                    if o.name != c.name {
                        out.push(match self {
                            Dialect::SqlServer => format!(
                                "EXEC sp_rename {}, {}, 'COLUMN'",
                                self.string_literal(&format!("{}.{}", old.name, o.name)),
                                self.string_literal(&c.name)
                            ),
                            _ => format!("ALTER TABLE {t} RENAME COLUMN {} TO {}", self.quote(&o.name), self.quote(&c.name)),
                        });
                    }
                    let changed = o.data_type != c.data_type || o.not_null != c.not_null || o.default != c.default || o.comment != c.comment || o.auto_increment != c.auto_increment;
                    if changed {
                        let q = self.quote(&c.name);
                        match self {
                            Dialect::MySql => out.push(format!("ALTER TABLE {t} MODIFY COLUMN {}", self.column_sql(c, false))),
                            Dialect::SqlServer => out.push(format!("ALTER TABLE {t} ALTER COLUMN {q} {} {}", c.data_type, if c.not_null { "NOT NULL" } else { "NULL" })),
                            Dialect::Postgres => {
                                if o.data_type != c.data_type {
                                    out.push(format!("ALTER TABLE {t} ALTER COLUMN {q} TYPE {}", c.data_type));
                                }
                                if o.not_null != c.not_null {
                                    out.push(format!("ALTER TABLE {t} ALTER COLUMN {q} {} NOT NULL", if c.not_null { "SET" } else { "DROP" }));
                                }
                                if o.default != c.default {
                                    out.push(if c.default.trim().is_empty() {
                                        format!("ALTER TABLE {t} ALTER COLUMN {q} DROP DEFAULT")
                                    } else {
                                        format!("ALTER TABLE {t} ALTER COLUMN {q} SET DEFAULT {}", c.default.trim())
                                    });
                                }
                                if o.comment != c.comment {
                                    out.push(format!("COMMENT ON COLUMN {t}.{q} IS {}", self.string_literal(&c.comment)));
                                }
                            }
                            Dialect::Sqlite => {}
                        }
                    }
                }
            }
        }
        let old_pk: Vec<&str> = old.columns.iter().filter(|c| c.primary_key).map(|c| c.name.as_str()).collect();
        let new_pk: Vec<&str> = new.columns.iter().filter(|c| c.primary_key).map(|c| c.name.as_str()).collect();
        if old_pk != new_pk {
            let cols = new_pk.iter().map(|c| self.quote(c)).collect::<Vec<_>>().join(", ");
            match self {
                Dialect::MySql => {
                    if !old_pk.is_empty() {
                        out.push(format!("ALTER TABLE {t} DROP PRIMARY KEY"));
                    }
                }
                Dialect::Postgres => out.push(format!("ALTER TABLE {t} DROP CONSTRAINT IF EXISTS {}", self.quote(&format!("{}_pkey", old.name)))),
                _ => out.push("-- SQL Server: rimuovere manualmente il vincolo PRIMARY KEY esistente".into()),
            }
            if !new_pk.is_empty() {
                out.push(format!("ALTER TABLE {t} ADD PRIMARY KEY ({cols})"));
            }
        }
        for i in &new.indexes {
            if !old.indexes.contains(i) {
                out.push(self.index_sql(new, i));
            }
        }
        for f in &new.foreign_keys {
            if !old.foreign_keys.contains(f) {
                out.push(format!("ALTER TABLE {t} ADD {}", self.fk_sql(f)));
            }
        }
        if old.name != new.name {
            out.push(match self {
                Dialect::MySql => format!("RENAME TABLE {t} TO {}", self.qualified(new.schema.as_deref(), &new.name)),
                Dialect::SqlServer => format!("EXEC sp_rename {}, {}", self.string_literal(&old.name), self.string_literal(&new.name)),
                _ => format!("ALTER TABLE {t} RENAME TO {}", self.quote(&new.name)),
            });
        }
        out
    }

    /// SQLite can't alter most of a table: build the new one, copy the kept columns, swap.
    fn sqlite_rebuild(self, old: &TableDesign, new: &TableDesign) -> Vec<String> {
        let only_adds = old.foreign_keys == new.foreign_keys
            && old.name == new.name
            && old.columns.len() <= new.columns.len()
            && old.columns.iter().zip(&new.columns).all(|(o, n)| {
                n.original.as_deref() == Some(o.name.as_str()) && ColumnDef { original: None, ..o.clone() } == ColumnDef { original: None, ..n.clone() }
            });
        if only_adds {
            let t = self.quote(&old.name);
            let mut out: Vec<String> = new.columns[old.columns.len()..].iter().map(|c| format!("ALTER TABLE {t} ADD COLUMN {}", self.column_sql(c, false))).collect();
            for i in &old.indexes {
                if !new.indexes.contains(i) {
                    out.push(self.drop_index_sql(old, &i.name));
                }
            }
            for i in &new.indexes {
                if !old.indexes.contains(i) {
                    out.push(self.index_sql(new, i));
                }
            }
            return out;
        }
        let tmp = format!("_dbcraft_new_{}", new.name);
        let mut tmp_design = new.clone();
        tmp_design.name = tmp.clone();
        tmp_design.indexes.clear();
        let mut out = vec!["PRAGMA foreign_keys = OFF".to_string()];
        out.extend(self.create_table(&tmp_design));
        let kept: Vec<(&str, &str)> = new
            .columns
            .iter()
            .filter_map(|c| c.original.as_deref().filter(|o| old.columns.iter().any(|x| x.name == *o)).map(|o| (o, c.name.as_str())))
            .collect();
        if !kept.is_empty() {
            out.push(format!(
                "INSERT INTO {} ({}) SELECT {} FROM {}",
                self.quote(&tmp),
                kept.iter().map(|(_, n)| self.quote(n)).collect::<Vec<_>>().join(", "),
                kept.iter().map(|(o, _)| self.quote(o)).collect::<Vec<_>>().join(", "),
                self.quote(&old.name)
            ));
        }
        out.push(format!("DROP TABLE {}", self.quote(&old.name)));
        out.push(format!("ALTER TABLE {} RENAME TO {}", self.quote(&tmp), self.quote(&new.name)));
        for i in &new.indexes {
            out.push(self.index_sql(new, i));
        }
        out.push("PRAGMA foreign_keys = ON".into());
        out
    }

    pub fn drop_table(self, schema: Option<&str>, name: &str, view: bool) -> String {
        format!("DROP {} {}", if view { "VIEW" } else { "TABLE" }, self.qualified(schema, name))
    }

    pub fn truncate_table(self, schema: Option<&str>, name: &str) -> String {
        match self {
            Dialect::Sqlite => format!("DELETE FROM {}", self.qualified(schema, name)),
            _ => format!("TRUNCATE TABLE {}", self.qualified(schema, name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str, pk: bool) -> ColumnDef {
        ColumnDef { name: name.into(), data_type: ty.into(), primary_key: pk, original: Some(name.into()), ..Default::default() }
    }

    #[test]
    fn quoting() {
        assert_eq!(Dialect::MySql.quote("a`b"), "`a``b`");
        assert_eq!(Dialect::SqlServer.quote("a]b"), "[a]]b]");
        assert_eq!(Dialect::Postgres.quote("a\"b"), "\"a\"\"b\"");
        assert_eq!(Dialect::Postgres.literal(&Value::Text("it's".into())), "'it''s'");
    }

    #[test]
    fn paging() {
        let r = PageRequest { table: "t".into(), limit: 10, offset: 20, ..Default::default() };
        assert_eq!(Dialect::Sqlite.select_page(&r), "SELECT * FROM \"t\" LIMIT 10 OFFSET 20");
        assert!(Dialect::SqlServer.select_page(&r).ends_with("ORDER BY (SELECT NULL) OFFSET 20 ROWS FETCH NEXT 10 ROWS ONLY"));
    }

    #[test]
    fn update_by_key() {
        let ch = RowChange::Update { key: vec![("id".into(), Value::Int(3))], set: vec![("name".into(), Value::Null)] };
        assert_eq!(Dialect::MySql.change_sql(None, "t", &ch), "UPDATE `t` SET `name` = NULL WHERE `id` = 3");
    }

    #[test]
    fn create_sqlite_autoincrement() {
        let mut id = col("id", "INTEGER", true);
        id.auto_increment = true;
        let d = TableDesign { name: "t".into(), columns: vec![id, col("n", "TEXT", false)], ..Default::default() };
        let sql = Dialect::Sqlite.create_table(&d);
        assert!(sql[0].contains("\"id\" INTEGER PRIMARY KEY AUTOINCREMENT"), "{}", sql[0]);
        assert!(!sql[0].contains("PRIMARY KEY (\"id\")"));
    }

    #[test]
    fn alter_mysql_add_and_rename() {
        let old = TableDesign { name: "t".into(), columns: vec![col("id", "int", true), col("a", "int", false)], ..Default::default() };
        let mut new = old.clone();
        new.columns[1].name = "b".into();
        new.columns.push(ColumnDef { name: "c".into(), data_type: "text".into(), ..Default::default() });
        let sql = Dialect::MySql.alter_table(&old, &new);
        assert_eq!(sql, vec!["ALTER TABLE `t` RENAME COLUMN `a` TO `b`", "ALTER TABLE `t` ADD COLUMN `c` text NULL"]);
    }

    #[test]
    fn alter_sqlite_rebuilds_on_type_change() {
        let old = TableDesign { name: "t".into(), columns: vec![col("id", "INTEGER", true), col("a", "INTEGER", false)], ..Default::default() };
        let mut new = old.clone();
        new.columns[1].data_type = "TEXT".into();
        let sql = Dialect::Sqlite.alter_table(&old, &new);
        assert!(sql.iter().any(|s| s.starts_with("INSERT INTO \"_dbcraft_new_t\" (\"id\", \"a\") SELECT \"id\", \"a\" FROM \"t\"")), "{sql:?}");
        assert!(sql.iter().any(|s| s == "ALTER TABLE \"_dbcraft_new_t\" RENAME TO \"t\""));
    }
}
