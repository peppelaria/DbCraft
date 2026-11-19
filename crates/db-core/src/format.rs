//! "Beautify SQL": re-indents a script with one clause per line, list items and `AND`/`OR`
//! conditions on their own lines, subqueries indented and keywords in upper case. Literals,
//! quoted identifiers and comments are kept as they are.

use crate::dialect::Dialect;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Space,
    Word(String),
    /// Strings, quoted identifiers, numbers, dollar-quoted bodies: copied verbatim.
    Lit(String),
    LineComment(String),
    BlockComment(String),
    Op(String),
}

pub(crate) fn tokenize(sql: &str, d: Dialect) -> Vec<Tok> {
    let chars: Vec<char> = sql.chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    let text = |a: usize, b: usize| chars[a..b.min(n)].iter().collect::<String>();
    while i < n {
        let c = chars[i];
        let start = i;
        if c.is_whitespace() {
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(Tok::Space);
            continue;
        }
        let next = chars.get(i + 1).copied();
        if (c == '-' && next == Some('-')) || (c == '#' && d == Dialect::MySql) {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            out.push(Tok::LineComment(text(start, i).trim_end().to_string()));
            continue;
        }
        if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < n && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            out.push(Tok::BlockComment(text(start, i)));
            continue;
        }
        let quote = match c {
            '\'' | '"' => Some(c),
            '`' if d == Dialect::MySql => Some('`'),
            '[' if d == Dialect::SqlServer => Some(']'),
            _ => None,
        };
        if let Some(close) = quote {
            i += 1;
            while i < n {
                if chars[i] == '\\' && c == '\'' && d == Dialect::MySql {
                    i += 2;
                    continue;
                }
                if chars[i] == close {
                    if chars.get(i + 1) == Some(&close) {
                        i += 2;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            out.push(Tok::Lit(text(start, i)));
            continue;
        }
        if c == '$' && d == Dialect::Postgres {
            let tag_end = (i + 1..n).find(|&k| !(chars[k].is_alphanumeric() || chars[k] == '_'));
            if let Some(e) = tag_end.filter(|&e| chars[e] == '$') {
                let tag: String = text(i, e + 1);
                let rest: String = text(e + 1, n);
                let end = rest.find(&tag).map_or(n, |p| e + 1 + rest[..p].chars().count() + tag.chars().count());
                out.push(Tok::Lit(text(start, end)));
                i = end;
                continue;
            }
        }
        if c.is_ascii_digit() || (c == '.' && next.is_some_and(|x| x.is_ascii_digit())) {
            while i < n && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || ((chars[i] == '+' || chars[i] == '-') && matches!(chars[i - 1], 'e' | 'E'))) {
                i += 1;
            }
            out.push(Tok::Lit(text(start, i)));
            continue;
        }
        if c.is_alphanumeric() || c == '_' || c == '@' || (c == '$' && d != Dialect::Postgres) {
            while i < n && (chars[i].is_alphanumeric() || matches!(chars[i], '_' | '$' | '@' | '#')) {
                i += 1;
            }
            out.push(Tok::Word(text(start, i)));
            continue;
        }
        let three = text(i, i + 3);
        let two = text(i, i + 2);
        let op = if three == "->>" {
            three
        } else if ["<=", ">=", "<>", "!=", "||", "::", ":=", "->", "=>"].contains(&two.as_str()) {
            two
        } else {
            c.to_string()
        };
        i += op.chars().count();
        out.push(Tok::Op(op));
    }
    out
}

const KEYWORDS: &[&str] = &[
    "ADD", "ALL", "ALTER", "AND", "ANY", "AS", "ASC", "AUTO_INCREMENT", "BEGIN", "BETWEEN", "BIGINT", "BOOLEAN", "BY", "CASCADE", "CASE", "CAST", "CHAR", "CHECK", "COLLATE",
    "COLUMN", "COMMIT", "CONSTRAINT", "CREATE", "CROSS", "CURRENT_DATE", "CURRENT_TIMESTAMP", "DATABASE", "DATE", "DATETIME", "DECIMAL", "DEFAULT", "DELETE", "DESC",
    "DISTINCT", "DROP", "ELSE", "END", "ESCAPE", "EXCEPT", "EXISTS", "EXPLAIN", "FALSE", "FETCH", "FIRST", "FOREIGN", "FROM", "FULL", "FUNCTION", "GROUP", "HAVING",
    "IF", "ILIKE", "IN", "INDEX", "INNER", "INSERT", "INT", "INTEGER", "INTERSECT", "INTERVAL", "INTO", "IS", "JOIN", "KEY", "LATERAL", "LEFT", "LIKE", "LIMIT", "NEXT",
    "NOT", "NULL", "NULLS", "NUMERIC", "OFFSET", "ON", "ONLY", "OR", "ORDER", "OUTER", "OVER", "PARTITION", "PRIMARY", "PROCEDURE", "RECURSIVE", "REFERENCES",
    "REPLACE", "RETURNING", "RIGHT", "ROLLBACK", "ROW", "ROWS", "SCHEMA", "SELECT", "SET", "SMALLINT", "TABLE", "TEXT", "THEN", "TIME", "TIMESTAMP", "TOP", "TRIGGER",
    "TRUE", "TRUNCATE", "UNION", "UNIQUE", "UNSIGNED", "UPDATE", "USING", "VALUES", "VARCHAR", "VIEW", "WHEN", "WHERE", "WINDOW", "WITH",
];

/// The SQL keywords the formatter upper-cases (sorted), also offered by code completion.
pub fn keywords() -> &'static [&'static str] {
    KEYWORDS
}

pub fn is_keyword(w: &str) -> bool {
    KEYWORDS.binary_search(&w.to_ascii_uppercase().as_str()).is_ok()
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Clause {
    None,
    /// Comma separated: one item per line.
    List,
    /// `AND`/`OR` on their own lines.
    Cond,
    /// `JOIN`s on their own lines, commas too.
    From,
    Create,
    Other,
}

#[derive(Clone, Copy)]
struct Frame {
    /// Indent of this frame's clause keywords.
    base: usize,
    /// Indent of the closing parenthesis.
    close: usize,
    clause: Clause,
    /// Plain parentheses open inside this frame.
    parens: usize,
    between: bool,
}

struct Out {
    s: String,
    level: usize,
}

impl Out {
    fn nl(&mut self, level: usize) {
        while self.s.ends_with(' ') {
            self.s.pop();
        }
        if !self.s.is_empty() && !self.s.ends_with('\n') {
            self.s.push('\n');
        }
        if !self.s.is_empty() {
            self.s.push_str(&"  ".repeat(level));
        }
        self.level = level;
    }
    fn at_line_start(&self) -> bool {
        self.s.is_empty() || self.s.trim_end_matches(' ').ends_with('\n')
    }
    fn space(&mut self) {
        if !self.s.is_empty() && !self.s.ends_with([' ', '\n', '(']) {
            self.s.push(' ');
        }
    }
}

/// Formats `sql`; returns it unchanged if it holds nothing but whitespace.
pub fn format_sql(sql: &str, d: Dialect) -> String {
    let toks = tokenize(sql, d);
    // Significant tokens, each with "had whitespace before it".
    let mut sig: Vec<(Tok, bool)> = Vec::new();
    let mut space = false;
    for t in toks {
        if t == Tok::Space {
            space = true;
        } else {
            sig.push((t, space));
            space = false;
        }
    }
    if sig.is_empty() {
        return sql.to_string();
    }
    let upper = |k: usize| match sig.get(k) {
        Some((Tok::Word(w), _)) => w.to_ascii_uppercase(),
        _ => String::new(),
    };
    let root = Frame { base: 0, close: 0, clause: Clause::None, parens: 0, between: false };
    let mut frames = vec![root];
    let mut o = Out { s: String::new(), level: 0 };
    // Put the next token on a new line at this level.
    let mut pending: Option<usize> = None;
    let mut force_space = false;
    let mut stmt_start = true;
    let mut i = 0;
    while i < sig.len() {
        let (tok, had_space) = sig[i].clone();
        let f = *frames.last().unwrap_or(&root);
        let prev_dot = i > 0 && sig[i - 1].0 == Tok::Op(".".into());
        let next_dot = sig.get(i + 1).is_some_and(|t| t.0 == Tok::Op(".".into()));
        let spacing = |o: &mut Out, pending: &mut Option<usize>, force: bool| {
            if let Some(l) = pending.take() {
                o.nl(l);
            } else if had_space || force {
                o.space();
            }
        };
        match tok {
            Tok::Word(w) if !prev_dot && !next_dot => {
                let u = w.to_ascii_uppercase();
                let mut phrase = vec![u.clone()];
                let mut j = i + 1;
                let take = |words: &[&str], phrase: &mut Vec<String>, j: &mut usize| {
                    if words.contains(&upper(*j).as_str()) {
                        phrase.push(upper(*j));
                        *j += 1;
                        true
                    } else {
                        false
                    }
                };
                match u.as_str() {
                    "GROUP" | "ORDER" | "PARTITION" => {
                        take(&["BY"], &mut phrase, &mut j);
                    }
                    "UNION" => {
                        take(&["ALL", "DISTINCT"], &mut phrase, &mut j);
                    }
                    "INSERT" | "REPLACE" => {
                        take(&["INTO"], &mut phrase, &mut j);
                    }
                    "DELETE" => {
                        take(&["FROM"], &mut phrase, &mut j);
                    }
                    "LEFT" | "RIGHT" | "FULL" => {
                        if take(&["OUTER"], &mut phrase, &mut j) || upper(j) == "JOIN" {
                            take(&["JOIN"], &mut phrase, &mut j);
                        }
                    }
                    "INNER" | "CROSS" | "NATURAL" => {
                        take(&["JOIN"], &mut phrase, &mut j);
                    }
                    _ => {}
                }
                let text = phrase.join(" ");
                let top = f.parens == 0;
                let clause = match text.as_str() {
                    "SELECT" | "GROUP BY" | "ORDER BY" | "VALUES" | "RETURNING" => Some(Clause::List),
                    "SET" if top && f.clause != Clause::Create => Some(Clause::List),
                    "FROM" | "USING" if f.clause != Clause::Create => Some(Clause::From),
                    "WHERE" | "HAVING" => Some(Clause::Cond),
                    "UPDATE" | "INSERT INTO" | "REPLACE INTO" | "DELETE FROM" | "DELETE" | "WITH" | "LIMIT" | "OFFSET" | "FETCH" => Some(Clause::Other),
                    "UNION" | "UNION ALL" | "UNION DISTINCT" | "INTERSECT" | "EXCEPT" => Some(Clause::None),
                    "CREATE" if stmt_start => Some(Clause::Create),
                    _ => None,
                };
                let is_join = text.ends_with("JOIN");
                let is_join = is_join && phrase.iter().all(|p| is_keyword(p) || p == "NATURAL");
                if let (Some(cl), true) = (clause, top) {
                    if !o.at_line_start() || pending.is_some() {
                        o.nl(f.base);
                    }
                    o.s.push_str(&text);
                    if let Some(fr) = frames.last_mut() {
                        fr.clause = cl;
                        fr.between = false;
                    }
                    pending = match cl {
                        Clause::List | Clause::From | Clause::Cond => Some(f.base + 1),
                        Clause::None => Some(f.base),
                        _ => None,
                    };
                    force_space = true;
                    // SELECT DISTINCT / TOP n stay on the SELECT line.
                    if text == "SELECT" {
                        while matches!(upper(j).as_str(), "DISTINCT" | "ALL" | "TOP") {
                            o.s.push(' ');
                            o.s.push_str(&upper(j));
                            if upper(j) == "TOP"
                                && let Some((Tok::Lit(n), _)) = sig.get(j + 1)
                            {
                                {
                                    o.s.push(' ');
                                    o.s.push_str(n);
                                    j += 1;
                                }
                            }
                            j += 1;
                        }
                    }
                    i = j;
                    stmt_start = false;
                    continue;
                }
                if is_join && top && f.clause == Clause::From {
                    pending = None;
                    o.nl(f.base + 1);
                    o.s.push_str(&text);
                    force_space = true;
                    i = j;
                    continue;
                }
                if (u == "AND" || u == "OR") && top && f.clause == Clause::Cond {
                    if u == "AND" && f.between {
                        if let Some(fr) = frames.last_mut() {
                            fr.between = false;
                        }
                    } else {
                        pending = None;
                        o.nl(f.base + 1);
                        o.s.push_str(&u);
                        force_space = true;
                        i += 1;
                        continue;
                    }
                }
                if u == "BETWEEN"
                    && let Some(fr) = frames.last_mut()
                {
                    fr.between = true;
                }
                spacing(&mut o, &mut pending, std::mem::take(&mut force_space));
                o.s.push_str(&if is_keyword(&w) { u } else { w });
                stmt_start = false;
            }
            Tok::Word(w) | Tok::Lit(w) | Tok::BlockComment(w) => {
                let after_dot = prev_dot;
                spacing(&mut o, &mut pending, std::mem::take(&mut force_space) && !after_dot);
                if after_dot && o.s.ends_with(' ') {
                    o.s.pop();
                }
                o.s.push_str(&w);
                stmt_start = false;
            }
            Tok::LineComment(c) => {
                if let Some(l) = pending.take() {
                    o.nl(l);
                } else {
                    o.space();
                }
                o.s.push_str(&c);
                pending = Some(o.level);
            }
            Tok::Op(op) => match op.as_str() {
                "," => {
                    pending = None;
                    while o.s.ends_with(' ') {
                        o.s.pop();
                    }
                    o.s.push(',');
                    let breaks = f.parens == 0 && matches!(f.clause, Clause::List | Clause::From | Clause::Create);
                    if breaks {
                        pending = Some(f.base + 1);
                    } else {
                        force_space = true;
                    }
                }
                "(" => {
                    let sub = matches!(upper(i + 1).as_str(), "SELECT" | "WITH");
                    let list = !sub && f.parens == 0 && f.clause == Clause::Create;
                    spacing(&mut o, &mut pending, std::mem::take(&mut force_space));
                    o.s.push('(');
                    if sub {
                        let content = if o.at_line_start() { o.level } else { o.level + 1 };
                        frames.push(Frame { base: content, close: content.saturating_sub(1), clause: Clause::None, parens: 0, between: false });
                        pending = Some(content);
                    } else if list {
                        frames.push(Frame { base: f.base, close: f.base, clause: Clause::List, parens: 0, between: false });
                        pending = Some(f.base + 1);
                    } else if let Some(fr) = frames.last_mut() {
                        fr.parens += 1;
                    }
                }
                ")" => {
                    pending = None;
                    if f.parens > 0 {
                        if let Some(fr) = frames.last_mut() {
                            fr.parens -= 1;
                        }
                        while o.s.ends_with(' ') {
                            o.s.pop();
                        }
                    } else if frames.len() > 1 {
                        frames.pop();
                        o.nl(f.close);
                    }
                    o.s.push(')');
                }
                ";" => {
                    while o.s.ends_with([' ', '\n']) {
                        o.s.pop();
                    }
                    o.s.push_str(";\n\n");
                    frames = vec![root];
                    pending = None;
                    force_space = false;
                    stmt_start = true;
                    o.level = 0;
                }
                "." => {
                    pending = None;
                    while o.s.ends_with(' ') {
                        o.s.pop();
                    }
                    o.s.push('.');
                }
                "::" => {
                    o.s.push_str("::");
                }
                _ => {
                    spacing(&mut o, &mut pending, std::mem::take(&mut force_space));
                    o.s.push_str(&op);
                }
            },
            Tok::Space => {}
        }
        i += 1;
    }
    let mut s = o.s.trim_end().to_string();
    s.push('\n');
    s.trim_end_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> String {
        format_sql(s, Dialect::Postgres)
    }

    #[test]
    fn keywords_sorted_for_binary_search() {
        let mut v = KEYWORDS.to_vec();
        v.sort_unstable();
        assert_eq!(v, KEYWORDS);
    }

    #[test]
    fn select_clauses_and_lists() {
        assert_eq!(
            f("select a, b.c as x, count(*) from t join u on u.id=t.uid where a = 1 and b between 1 and 2 or c like 'x, y' group by a, b order by a desc limit 10"),
            "SELECT\n  a,\n  b.c AS x,\n  count(*)\nFROM\n  t\n  JOIN u ON u.id=t.uid\nWHERE\n  a = 1\n  AND b BETWEEN 1 AND 2\n  OR c LIKE 'x, y'\nGROUP BY\n  a,\n  b\nORDER BY\n  a DESC\nLIMIT 10"
        );
    }

    #[test]
    fn subqueries_and_distinct() {
        assert_eq!(
            f("SELECT DISTINCT id FROM t WHERE id IN (select x from u where y > 2) AND z = 1"),
            "SELECT DISTINCT\n  id\nFROM\n  t\nWHERE\n  id IN (\n    SELECT\n      x\n    FROM\n      u\n    WHERE\n      y > 2\n  )\n  AND z = 1"
        );
    }

    #[test]
    fn statements_comments_and_literals() {
        let out = f("insert into t (a,b) values (1,'it''s; ok'),(2,$$x$$); -- note\nupdate t set a=1, b = 2 where id=3;");
        assert_eq!(out, "INSERT INTO t (a, b)\nVALUES\n  (1, 'it''s; ok'),\n  (2, $$x$$);\n\n-- note\nUPDATE t\nSET\n  a=1,\n  b = 2\nWHERE\n  id=3;");
        // Identifiers after a dot and quoted names keep their case.
        assert_eq!(f("select t.date, \"Order\" from s.t"), "SELECT\n  t.date,\n  \"Order\"\nFROM\n  s.t");
        assert_eq!(f("   "), "   ");
    }

    #[test]
    fn create_table_columns() {
        assert_eq!(
            f("create table t (id int primary key, name varchar(20) not null, price decimal(10,2))"),
            "CREATE TABLE t (\n  id INT PRIMARY KEY,\n  name VARCHAR(20) NOT NULL,\n  price DECIMAL(10, 2)\n)"
        );
    }

    #[test]
    fn mysql_and_sql_server_quoting() {
        assert_eq!(format_sql("select `a b`, x from `t` # c\nwhere 1", Dialect::MySql), "SELECT\n  `a b`,\n  x\nFROM\n  `t` # c\nWHERE\n  1");
        assert_eq!(format_sql("select top 5 [a,b] from t", Dialect::SqlServer), "SELECT TOP 5\n  [a,b]\nFROM\n  t");
    }
}
