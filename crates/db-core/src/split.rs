//! Splits an SQL script into statements on `;`, ignoring semicolons inside string literals,
//! quoted identifiers, comments and PostgreSQL dollar-quoted bodies.

use crate::dialect::Dialect;

pub fn split_statements(sql: &str, d: Dialect) -> Vec<String> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let push = |out: &mut Vec<String>, s: &str| {
        let t = s.trim();
        if !t.is_empty() && !is_only_comments(t) {
            out.push(t.to_string());
        }
    };
    while i < b.len() {
        let c = b[i];
        match c {
            b'\'' | b'"' | b'`' | b'[' => {
                let close = if c == b'[' { b']' } else { c };
                if c == b'[' && d != Dialect::SqlServer {
                    i += 1;
                    continue;
                }
                if c == b'`' && d != Dialect::MySql {
                    i += 1;
                    continue;
                }
                i += 1;
                while i < b.len() {
                    if b[i] == b'\\' && c == b'\'' && d == Dialect::MySql {
                        i += 2;
                        continue;
                    }
                    if b[i] == close {
                        // Doubled quote = escaped quote.
                        if b.get(i + 1) == Some(&close) {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'-' if b.get(i + 1) == Some(&b'-') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'#' if d == Dialect::MySql => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'$' if d == Dialect::Postgres => {
                // $tag$ … $tag$ (tag may be empty).
                let tag_end = b[i + 1..].iter().position(|x| *x == b'$').map(|p| i + 1 + p);
                match tag_end {
                    Some(e) if b[i + 1..e].iter().all(|x| x.is_ascii_alphanumeric() || *x == b'_') => {
                        let tag = &sql[i..=e];
                        match sql[e + 1..].find(tag) {
                            Some(p) => i = e + 1 + p + tag.len(),
                            None => i = b.len(),
                        }
                    }
                    _ => i += 1,
                }
            }
            b';' => {
                push(&mut out, &sql[start..i]);
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    if start < sql.len() {
        push(&mut out, &sql[start.min(sql.len())..]);
    }
    out
}

fn is_only_comments(s: &str) -> bool {
    s.lines().all(|l| {
        let l = l.trim();
        l.is_empty() || l.starts_with("--") || l.starts_with('#')
    }) || (s.starts_with("/*") && s.ends_with("*/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_semicolons_outside_literals() {
        let v = split_statements("SELECT 'a;b'; SELECT \"x;\" ; -- c;\nSELECT 1 /* ; */;", Dialect::Postgres);
        assert_eq!(v, vec!["SELECT 'a;b'", "SELECT \"x;\"", "-- c;\nSELECT 1 /* ; */"]);
        assert_eq!(split_statements("  ;; ", Dialect::Sqlite), Vec::<String>::new());
        assert_eq!(split_statements("SELECT 'it''s; ok'", Dialect::Sqlite), vec!["SELECT 'it''s; ok'"]);
    }

    #[test]
    fn mysql_backslash_backtick_and_hash() {
        let v = split_statements("SELECT 'a\\';b'; SELECT `c;d` # x;y\n; SELECT 2", Dialect::MySql);
        assert_eq!(v, vec!["SELECT 'a\\';b'", "SELECT `c;d` # x;y", "SELECT 2"]);
    }

    #[test]
    fn postgres_dollar_quotes() {
        let f = "CREATE FUNCTION f() RETURNS int AS $body$ BEGIN RETURN 1; END; $body$ LANGUAGE plpgsql; SELECT f()";
        let v = split_statements(f, Dialect::Postgres);
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v[0].ends_with("LANGUAGE plpgsql"));
        assert_eq!(split_statements("SELECT $$a;b$$; SELECT 2", Dialect::Postgres), vec!["SELECT $$a;b$$", "SELECT 2"]);
        // $1 parameters are not quotes.
        assert_eq!(split_statements("SELECT $1; SELECT 2", Dialect::Postgres).len(), 2);
    }

    #[test]
    fn sql_server_brackets() {
        assert_eq!(split_statements("SELECT [a;b]; SELECT 1", Dialect::SqlServer), vec!["SELECT [a;b]", "SELECT 1"]);
    }
}
