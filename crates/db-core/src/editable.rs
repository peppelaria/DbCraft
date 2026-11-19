//! Which query results can be edited: a `SELECT` that reads ONE table, with no join, grouping,
//! `DISTINCT` or `UNION`, so that every row of the result is one row of that table. Its plain
//! columns (`id`, `t.title`, `title AS t`, `*`) are mapped back to the table's columns; computed
//! ones (`UPPER(x)`, `a + b`, `x::text`) stay read-only.
//!
//! The parser is deliberately strict: whatever it doesn't recognise makes the result read-only,
//! never "probably editable".

use crate::dialect::Dialect;
use crate::format::{Tok, tokenize};

/// The table a `SELECT` reads, and its plain columns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectSource {
    /// The schema (or database, on MySQL) the table was qualified with.
    pub schema: Option<String>,
    pub table: String,
    /// The whole row is selected (`*` or `t.*`).
    pub star: bool,
    /// Plain columns: (name in the result, column of the table).
    pub columns: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
enum T {
    /// A bare word (keyword or unquoted identifier).
    Word(String),
    /// A quoted identifier, unquoted.
    Ident(String),
    /// Strings, numbers.
    Lit,
    Op(String),
}

/// Words that never name a column or a table when they appear unquoted.
const RESERVED: &[&str] = &[
    "ALL", "AND", "AS", "BETWEEN", "BINARY", "BY", "CASE", "COLLATE", "CROSS", "CURRENT_DATE", "CURRENT_TIME", "CURRENT_TIMESTAMP", "CURRENT_USER", "DISTINCT",
    "DISTINCTROW", "ELSE", "END", "EXCEPT", "EXISTS", "FALSE", "FETCH", "FOR", "FROM", "FULL", "GROUP", "HAVING", "HIGH_PRIORITY", "IN", "INNER", "INTERSECT",
    "INTERVAL", "INTO", "IS", "JOIN", "LATERAL", "LEFT", "LIKE", "LIMIT", "LOCALTIME", "LOCALTIMESTAMP", "MINUS", "NATURAL", "NOT", "NULL", "OFFSET", "ON", "OR",
    "ORDER", "OUTER", "OVER", "PROCEDURE", "RIGHT", "SELECT", "SQL_BIG_RESULT", "SQL_BUFFER_RESULT", "SQL_CACHE", "SQL_CALC_FOUND_ROWS", "SQL_NO_CACHE",
    "SQL_SMALL_RESULT", "STRAIGHT_JOIN", "THEN", "TOP", "TRUE", "UNION", "USING", "VALUES", "WHEN", "WHERE", "WINDOW", "WITH",
];

fn reserved(w: &str) -> bool {
    RESERVED.iter().any(|r| r.eq_ignore_ascii_case(w))
}

fn tokens(sql: &str, d: Dialect) -> Vec<T> {
    tokenize(sql, d)
        .into_iter()
        .filter_map(|t| match t {
            Tok::Space | Tok::LineComment(_) | Tok::BlockComment(_) => None,
            Tok::Word(w) => Some(T::Word(w)),
            Tok::Op(o) => Some(T::Op(o)),
            Tok::Lit(l) => Some(match unquote(&l, d) {
                Some(id) => T::Ident(id),
                None => T::Lit,
            }),
        })
        .collect()
}

/// The name inside a quoted identifier (`"x"`, `` `x` ``, `[x]`); `None` for strings and numbers.
fn unquote(lit: &str, d: Dialect) -> Option<String> {
    let mut chars = lit.chars();
    let (open, close) = match (chars.next()?, d) {
        ('`', Dialect::MySql) => ('`', '`'),
        ('[', Dialect::SqlServer) => ('[', ']'),
        // on MySQL "…" is a string
        ('"', Dialect::Postgres | Dialect::Sqlite | Dialect::SqlServer) => ('"', '"'),
        _ => return None,
    };
    let inner = lit.strip_prefix(open)?.strip_suffix(close)?;
    let doubled: String = [close, close].iter().collect();
    Some(inner.replace(&doubled, &close.to_string()))
}

/// An identifier: a quoted one, or a bare word that is not a keyword (nor a variable).
fn ident(t: Option<&T>) -> Option<String> {
    match t? {
        T::Ident(s) => Some(s.clone()),
        T::Word(w) if !reserved(w) && !w.starts_with('@') && !w.starts_with('$') => Some(w.clone()),
        _ => None,
    }
}

fn is_word(t: Option<&T>, w: &str) -> bool {
    matches!(t, Some(T::Word(x)) if x.eq_ignore_ascii_case(w))
}

fn is_op(t: Option<&T>, o: &str) -> bool {
    matches!(t, Some(T::Op(x)) if x == o)
}

/// One item of the select list.
enum ItemKind {
    /// `*` or `q.*`.
    Star(Option<String>),
    /// A column, with its qualifier (table or alias) and the name it gets in the result.
    Column { qualifier: Option<String>, column: String, name: String },
    /// Anything else.
    Computed,
}

fn select_item(t: &[T]) -> ItemKind {
    if let [T::Op(o)] = t
        && o == "*"
    {
        return ItemKind::Star(None);
    }
    if let [q, T::Op(dot), T::Op(star)] = t
        && dot == "."
        && star == "*"
        && let Some(q) = ident(Some(q))
    {
        return ItemKind::Star(Some(q));
    }
    // a.b.c [AS] alias
    let mut parts = Vec::new();
    let mut i = 0;
    loop {
        let Some(p) = ident(t.get(i)) else { return ItemKind::Computed };
        parts.push(p);
        i += 1;
        if is_op(t.get(i), ".") && parts.len() < 3 {
            i += 1;
        } else {
            break;
        }
    }
    let alias = match &t[i..] {
        [] => None,
        [a] => match ident(Some(a)) {
            Some(a) => Some(a),
            None => return ItemKind::Computed,
        },
        [T::Word(as_), a] if as_.eq_ignore_ascii_case("AS") => match ident(Some(a)) {
            Some(a) => Some(a),
            None => return ItemKind::Computed,
        },
        _ => return ItemKind::Computed,
    };
    let Some(column) = parts.pop() else { return ItemKind::Computed };
    let qualifier = parts.pop();
    ItemKind::Column { qualifier, name: alias.unwrap_or_else(|| column.clone()), column }
}

/// The table and the plain columns of a single-table `SELECT`; `None` for anything else
/// (joins, `GROUP BY`, `DISTINCT`, `UNION`, CTEs, several statements…).
pub fn select_source(sql: &str, d: Dialect) -> Option<SelectSource> {
    let mut t = tokens(sql, d);
    while is_op(t.last(), ";") {
        t.pop();
    }
    if !is_word(t.first(), "SELECT") || t.iter().any(|x| is_op(Some(x), ";")) {
        return None;
    }
    let mut i = 1;
    if is_word(t.get(i), "ALL") {
        i += 1;
    }
    // SQL Server: TOP n / TOP (n)
    if d == Dialect::SqlServer && is_word(t.get(i), "TOP") {
        i += 1;
        if is_op(t.get(i), "(") && t.get(i + 1) == Some(&T::Lit) && is_op(t.get(i + 2), ")") {
            i += 3;
        } else if t.get(i) == Some(&T::Lit) {
            i += 1;
        } else {
            return None;
        }
    }
    // DISTINCT, STRAIGHT_JOIN, SQL_CALC_FOUND_ROWS… : the rows are not the table's rows (or unsure)
    if matches!(t.get(i), Some(T::Word(w)) if reserved(w)) {
        return None;
    }

    // the select list, up to FROM at depth 0
    let mut depth = 0i32;
    let mut items: Vec<&[T]> = Vec::new();
    let mut start = i;
    let from = loop {
        let tok = t.get(i)?;
        match tok {
            T::Op(o) if o == "(" => depth += 1,
            T::Op(o) if o == ")" => depth -= 1,
            T::Op(o) if o == "," && depth == 0 => {
                items.push(&t[start..i]);
                start = i + 1;
            }
            // SELECT … INTO writes somewhere else
            T::Word(w) if depth == 0 && w.eq_ignore_ascii_case("INTO") => return None,
            T::Word(w) if depth == 0 && w.eq_ignore_ascii_case("FROM") => {
                items.push(&t[start..i]);
                break i;
            }
            _ => {}
        }
        i += 1;
    };
    if items.iter().any(|x| x.is_empty()) {
        return None;
    }

    // FROM [schema.]table [[AS] alias]
    let mut i = from + 1;
    let mut names = vec![ident(t.get(i))?];
    i += 1;
    while is_op(t.get(i), ".") {
        names.push(ident(t.get(i + 1))?);
        i += 2;
    }
    if names.len() > 2 {
        return None;
    }
    let table = names.pop()?;
    let schema = names.pop();
    let mut alias = None;
    if is_word(t.get(i), "AS") {
        alias = Some(ident(t.get(i + 1))?);
        i += 2;
    } else if let Some(a) = ident(t.get(i)) {
        alias = Some(a);
        i += 1;
    }
    // what may follow: nothing, or WHERE / ORDER BY / LIMIT / OFFSET / FETCH / FOR UPDATE
    match t.get(i) {
        None => {}
        Some(T::Word(w)) if ["WHERE", "ORDER", "LIMIT", "OFFSET", "FETCH", "FOR"].iter().any(|k| k.eq_ignore_ascii_case(w)) => {}
        _ => return None,
    }
    let mut depth = 0i32;
    for tok in &t[i..] {
        match tok {
            T::Op(o) if o == "(" => depth += 1,
            T::Op(o) if o == ")" => depth -= 1,
            T::Word(w) if depth == 0 && ["GROUP", "HAVING", "UNION", "INTERSECT", "EXCEPT", "MINUS", "WINDOW", "INTO", "JOIN", "PROCEDURE"].iter().any(|k| k.eq_ignore_ascii_case(w)) => {
                return None;
            }
            _ => {}
        }
    }

    // the qualifier of a column must be the table (or its alias)
    let names_table = |q: &Option<String>| match q {
        None => true,
        Some(q) => match &alias {
            Some(a) => a.eq_ignore_ascii_case(q),
            None => table.eq_ignore_ascii_case(q),
        },
    };
    let mut src = SelectSource { schema, table: table.clone(), ..Default::default() };
    for item in items {
        match select_item(item) {
            ItemKind::Star(q) if names_table(&q) => src.star = true,
            ItemKind::Column { qualifier, column, name } if names_table(&qualifier) => src.columns.push((name, column)),
            _ => {}
        }
    }
    Some(src)
}

impl SelectSource {
    /// For each column of the result (by name), the table column it shows: `None` when it is
    /// computed, unknown to the table, or ambiguous (two result columns with the same name).
    pub fn map_columns(&self, result: &[String], table_columns: &[String]) -> Vec<Option<String>> {
        let table_col = |name: &str| {
            let mut found = table_columns.iter().filter(|c| c.eq_ignore_ascii_case(name));
            match (found.next(), found.next()) {
                (Some(c), None) => Some(c.clone()),
                _ => None,
            }
        };
        result
            .iter()
            .map(|name| {
                if result.iter().filter(|n| n.eq_ignore_ascii_case(name)).count() != 1 {
                    return None;
                }
                let mut explicit = self.columns.iter().filter(|(n, _)| n.eq_ignore_ascii_case(name));
                match (explicit.next(), explicit.next()) {
                    (Some((_, col)), None) => table_col(col),
                    (Some(_), Some(_)) => None,
                    (None, _) if self.star => table_col(name),
                    (None, _) => None,
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn src(sql: &str) -> Option<SelectSource> {
        select_source(sql, Dialect::MySql)
    }

    fn cols(s: &SelectSource) -> Vec<(&str, &str)> {
        s.columns.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
    }

    #[test]
    fn single_table_selects() {
        let s = src("SELECT\n  id, title, credits_cnf\nFROM courses\nORDER BY accredited_at DESC, id DESC\nLIMIT 200;").unwrap();
        assert_eq!((s.schema.as_deref(), s.table.as_str(), s.star), (None, "courses", false));
        assert_eq!(cols(&s), [("id", "id"), ("title", "title"), ("credits_cnf", "credits_cnf")]);

        let s = src("select c.*, UPPER(c.title) AS t, c.title AS name, `order` from shop.`courses` as c where c.id in (select 1 from x join y) for update").unwrap();
        assert_eq!((s.schema.as_deref(), s.table.as_str(), s.star), (Some("shop"), "courses", true));
        assert_eq!(cols(&s), [("name", "title"), ("order", "order")]);

        let s = select_source("SELECT TOP (10) [id], \"Title\" t FROM dbo.[my table] WHERE 1 = 1", Dialect::SqlServer).unwrap();
        assert_eq!((s.schema.as_deref(), s.table.as_str()), (Some("dbo"), "my table"));
        assert_eq!(cols(&s), [("id", "id"), ("t", "Title")]);

        // a column of another table is not mapped
        let s = src("SELECT x.id, id FROM t").unwrap();
        assert_eq!(cols(&s), [("id", "id")]);
        // a string is not a column on MySQL
        assert!(cols(&src("SELECT \"id\" FROM t").unwrap()).is_empty());
    }

    #[test]
    fn everything_else_is_read_only() {
        for sql in [
            "SELECT DISTINCT id FROM t",
            "SELECT id FROM a JOIN b ON a.id = b.id",
            "SELECT id FROM a, b",
            "SELECT id FROM a LEFT JOIN b USING (id)",
            "SELECT city, count(*) FROM t GROUP BY city",
            "SELECT id FROM t UNION SELECT id FROM u",
            "SELECT id FROM (SELECT id FROM t) x",
            "WITH x AS (SELECT 1) SELECT * FROM x",
            "SELECT id FROM t; SELECT 1",
            "SELECT 1",
            "UPDATE t SET a = 1",
            "SELECT id FROM generate_series(1, 3)",
            "SELECT id INTO @x FROM t",
            "SELECT id FROM a.b.c",
            "SELECT SQL_CALC_FOUND_ROWS id FROM t",
        ] {
            assert_eq!(src(sql), None, "{sql}");
        }
    }

    #[test]
    fn result_columns_map_to_table_columns() {
        let table = ["id".to_string(), "title".into(), "price".into()];
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let s = src("SELECT id, title AS name, price * 2 AS price FROM t").unwrap();
        assert_eq!(s.map_columns(&names(&["id", "name", "price"]), &table), [Some("id".into()), Some("title".into()), None]);
        // `*` plus a computed column named like a real one: both ambiguous
        let s = src("SELECT *, UPPER(title) AS title FROM t").unwrap();
        assert_eq!(s.map_columns(&names(&["id", "title", "price", "title"]), &table), [Some("id".into()), None, Some("price".into()), None]);
        // swapped names
        let s = src("SELECT title AS id, id AS title FROM t").unwrap();
        assert_eq!(s.map_columns(&names(&["id", "title"]), &table), [Some("title".into()), Some("id".into())]);
        // a column the table doesn't have
        let s = src("SELECT id, nope FROM t").unwrap();
        assert_eq!(s.map_columns(&names(&["id", "nope"]), &table), [Some("id".into()), None]);
    }
}
