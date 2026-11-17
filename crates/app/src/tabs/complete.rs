//! Code completion for the query editor: SQL keywords, the tables, views and routines of the
//! tab's database/schema, and columns — of the tables the statement names, or of the table (or
//! alias) before a `.`.

use std::collections::{HashMap, HashSet};

use db_core::format::{is_keyword, keywords};
use db_core::{Dialect, ObjectKind, SchemaObject};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Column,
    Table,
    View,
    Function,
    Procedure,
    Keyword,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: String,
    pub kind: Kind,
    /// Shown on the right: the database for objects, the type for columns.
    pub detail: String,
}

/// What is being typed at the cursor.
#[derive(Debug, PartialEq)]
pub struct Context {
    /// Char index where the word under completion starts.
    pub start: usize,
    pub prefix: String,
    /// The name before a `.`, unquoted.
    pub qualifier: Option<String>,
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let inner = if s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('`') && s.ends_with('`')) || (s.starts_with('[') && s.ends_with(']'))) { &s[1..s.len() - 1] } else { s };
    inner.to_string()
}

/// The word before `cursor` (a char index) and its qualifier.
pub fn context(text: &str, cursor: usize) -> Context {
    let chars: Vec<char> = text.chars().take(cursor).collect();
    let mut start = chars.len();
    while start > 0 && ident_char(chars[start - 1]) {
        start -= 1;
    }
    let prefix: String = chars[start..].iter().collect();
    let mut qualifier = None;
    if start > 0 && chars[start - 1] == '.' {
        let end = start - 1;
        let mut q = end;
        match chars.get(end.wrapping_sub(1)) {
            Some(&c @ ('"' | '`' | ']')) => {
                let open = if c == ']' { '[' } else { c };
                q = end - 1;
                while q > 0 && chars[q - 1] != open {
                    q -= 1;
                }
                q = q.saturating_sub(1);
            }
            _ => {
                while q > 0 && ident_char(chars[q - 1]) {
                    q -= 1;
                }
            }
        }
        let name: String = chars[q..end].iter().collect();
        if !name.is_empty() {
            qualifier = Some(unquote(&name));
        }
    }
    Context { start, prefix, qualifier }
}

/// Table names (lower case) the statement refers to, keyed by alias or name (lower case).
pub fn table_refs(stmt: &str, is_table: &dyn Fn(&str) -> bool) -> HashMap<String, String> {
    // Identifiers (possibly quoted and dotted) and other tokens.
    let mut toks: Vec<String> = Vec::new();
    let chars: Vec<char> = stmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if ident_char(c) || matches!(c, '"' | '`' | '[') {
            let mut name = String::new();
            loop {
                match chars.get(i) {
                    Some(&q @ ('"' | '`' | '[')) => {
                        let close = if q == '[' { ']' } else { q };
                        i += 1;
                        while i < chars.len() && chars[i] != close {
                            name.push(chars[i]);
                            i += 1;
                        }
                        i += 1;
                    }
                    Some(&c) if ident_char(c) => {
                        while i < chars.len() && ident_char(chars[i]) {
                            name.push(chars[i]);
                            i += 1;
                        }
                    }
                    _ => break,
                }
                if chars.get(i) == Some(&'.') {
                    name.push('.');
                    i += 1;
                } else {
                    break;
                }
            }
            toks.push(name);
        } else if c == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i += 1;
            toks.push("''".into());
        } else {
            if !c.is_whitespace() {
                toks.push(c.to_string());
            }
            i += 1;
        }
    }
    let mut out = HashMap::new();
    for (k, t) in toks.iter().enumerate() {
        let table = t.rsplit('.').next().unwrap_or(t).to_lowercase();
        if table.is_empty() || !is_table(&table) {
            continue;
        }
        // Not a column reference like `t.col` or `alias.col`.
        if k > 0 && toks[k - 1] == "." {
            continue;
        }
        out.insert(table.clone(), table.clone());
        let mut n = k + 1;
        if toks.get(n).is_some_and(|w| w.eq_ignore_ascii_case("AS")) {
            n += 1;
        }
        if let Some(alias) = toks.get(n)
            && alias.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
            && !is_keyword(alias)
            && !matches!(alias.to_ascii_uppercase().as_str(), "LEFT" | "RIGHT" | "INNER" | "OUTER" | "FULL" | "CROSS" | "NATURAL" | "LIMIT" | "WINDOW")
        {
            out.insert(alias.to_lowercase(), table);
        }
    }
    out
}

/// Metadata for completion, filled in by the query tab.
#[derive(Default)]
pub struct Catalog {
    pub objects: Vec<SchemaObject>,
    /// Lower-case table name → (column, type).
    pub columns: HashMap<String, Vec<(String, String)>>,
    /// Tables whose columns were asked for.
    pub requested: HashSet<String>,
    /// Shown next to objects (database or schema).
    pub label: String,
}

impl Catalog {
    fn object(&self, lower: &str) -> Option<&SchemaObject> {
        self.objects.iter().find(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View) && o.name.to_lowercase() == lower)
    }

    /// Candidates for `ctx` in `stmt`, best first. Tables whose columns are needed but not loaded
    /// are added to `need` (lower case), also when already requested.
    pub fn candidates(&self, ctx: &Context, stmt: &str, forced: bool, need: &mut Vec<String>) -> Vec<Item> {
        let refs = table_refs(stmt, &|t| self.object(t).is_some());
        let mut items: Vec<Item> = Vec::new();
        let mut columns_of = |table: &str, items: &mut Vec<Item>| match self.columns.get(table) {
            Some(cols) => items.extend(cols.iter().map(|(n, t)| Item { label: n.clone(), kind: Kind::Column, detail: t.clone() })),
            None => {
                // Still to load (asked for again: the tab skips tables already requested).
                if !need.iter().any(|n| n == table) {
                    need.push(table.to_string());
                }
            }
        };
        if let Some(q) = &ctx.qualifier {
            let q = q.to_lowercase();
            let table = refs.get(&q).cloned().or_else(|| self.object(&q).map(|_| q.clone()));
            match table {
                Some(t) => columns_of(&t, &mut items),
                None => return Vec::new(),
            }
        } else {
            if ctx.prefix.is_empty() && !forced {
                return Vec::new();
            }
            let mut tables: Vec<&String> = refs.values().collect();
            tables.sort();
            tables.dedup();
            for t in tables {
                columns_of(t, &mut items);
            }
            items.extend(self.objects.iter().filter(|o| o.kind != ObjectKind::Trigger).map(|o| Item {
                label: o.name.clone(),
                kind: match o.kind {
                    ObjectKind::Table => Kind::Table,
                    ObjectKind::View => Kind::View,
                    ObjectKind::Function => Kind::Function,
                    _ => Kind::Procedure,
                },
                detail: self.label.clone(),
            }));
            items.extend(keywords().iter().map(|k| Item { label: (*k).to_string(), kind: Kind::Keyword, detail: String::new() }));
        }
        let p = ctx.prefix.to_lowercase();
        let mut ranked: Vec<(u8, Item)> = items
            .into_iter()
            .filter_map(|it| {
                let l = it.label.to_lowercase();
                let rank = if p.is_empty() || l.starts_with(&p) {
                    0
                } else if l.contains(&p) {
                    1
                } else {
                    return None;
                };
                Some((rank, it))
            })
            .collect();
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.kind.cmp(&b.1.kind)).then_with(|| a.1.label.to_lowercase().cmp(&b.1.label.to_lowercase())));
        ranked.dedup_by(|a, b| a.1.label == b.1.label && a.1.kind == b.1.kind);
        let mut out: Vec<Item> = ranked.into_iter().map(|(_, it)| it).take(300).collect();
        // Nothing left to complete.
        if out.len() == 1 && out[0].label.eq_ignore_ascii_case(&ctx.prefix) && !forced {
            out.clear();
        }
        out
    }
}

/// The text to insert for an item: quoted when the name needs it.
pub fn insert_text(it: &Item, d: Dialect) -> String {
    if it.kind == Kind::Keyword {
        return it.label.clone();
    }
    let plain = it.label.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && it.label.chars().all(|c| c.is_alphanumeric() || c == '_');
    let case_ok = d != Dialect::Postgres || !it.label.chars().any(|c| c.is_uppercase());
    if plain && case_ok && !is_keyword(&it.label) { it.label.clone() } else { d.quote(&it.label) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(name: &str, kind: ObjectKind) -> SchemaObject {
        SchemaObject { name: name.into(), kind, rows: None, comment: None }
    }

    fn catalog() -> Catalog {
        let mut c = Catalog { label: "crm".into(), ..Default::default() };
        c.objects = vec![obj("sessions", ObjectKind::Table), obj("users", ObjectKind::Table), obj("sectors", ObjectKind::Table), obj("v_active", ObjectKind::View), obj("score", ObjectKind::Function)];
        c.columns.insert("users".into(), vec![("id".into(), "int".into()), ("email".into(), "text".into()), ("session_id".into(), "int".into())]);
        c
    }

    #[test]
    fn context_at_cursor() {
        assert_eq!(context("SELECT SE", 9), Context { start: 7, prefix: "SE".into(), qualifier: None });
        assert_eq!(context("select u.em", 11), Context { start: 9, prefix: "em".into(), qualifier: Some("u".into()) });
        assert_eq!(context("select \"My T\".", 14), Context { start: 14, prefix: String::new(), qualifier: Some("My T".into()) });
        assert_eq!(context("é x", 3).prefix, "x");
    }

    #[test]
    fn aliases() {
        let is_t = |t: &str| ["users", "sessions"].contains(&t);
        let r = table_refs("SELECT * FROM public.users u JOIN \"sessions\" AS s ON s.id = u.session_id WHERE u.x = 'users'", &is_t);
        assert_eq!(r.get("u").map(String::as_str), Some("users"));
        assert_eq!(r.get("s").map(String::as_str), Some("sessions"));
        assert_eq!(r.get("users").map(String::as_str), Some("users"));
        let r = table_refs("SELECT * FROM users LEFT JOIN sessions ON 1=1 WHERE 1", &is_t);
        assert!(!r.contains_key("left") && !r.contains_key("on"), "{r:?}");
    }

    #[test]
    fn candidates_rank_prefix_first() {
        let c = catalog();
        let mut need = Vec::new();
        let ctx = context("SE", 2);
        let v = c.candidates(&ctx, "SE", false, &mut need);
        let labels: Vec<&str> = v.iter().map(|i| i.label.as_str()).collect();
        // Prefix matches (tables before keywords), then the ones that contain it.
        assert_eq!(&labels[..4], ["sectors", "sessions", "SELECT", "SET"]);
        assert!(labels.contains(&"users") && labels.contains(&"INSERT"));
        assert!(need.is_empty());
    }

    #[test]
    fn columns_through_alias_and_loading() {
        let c = catalog();
        let mut need = Vec::new();
        let sql = "SELECT u. FROM users u JOIN sessions s ON 1=1";
        let v = c.candidates(&context(sql, 9), sql, false, &mut need);
        assert_eq!(v.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(), ["email", "id", "session_id"]);
        // Columns of sessions are not loaded yet: asked for.
        let sql = "SELECT s.i FROM users u JOIN sessions s ON 1=1";
        assert!(c.candidates(&context(sql, 10), sql, false, &mut need).is_empty());
        assert_eq!(need, ["sessions"]);
        // Without a qualifier, columns of the statement's tables come first.
        let sql = "SELECT ema FROM users";
        let mut need = Vec::new();
        assert_eq!(c.candidates(&context(sql, 10), sql, false, &mut need)[0].label, "email");
        // Nothing typed: nothing offered unless asked (Ctrl+Space).
        assert!(c.candidates(&context("SELECT ", 7), "SELECT ", false, &mut need).is_empty());
        assert!(!c.candidates(&context("SELECT ", 7), "SELECT ", true, &mut need).is_empty());
    }

    #[test]
    fn quoting_on_insert() {
        let it = |l: &str, kind| Item { label: l.into(), kind, detail: String::new() };
        assert_eq!(insert_text(&it("users", Kind::Table), Dialect::Postgres), "users");
        assert_eq!(insert_text(&it("Users", Kind::Table), Dialect::Postgres), "\"Users\"");
        assert_eq!(insert_text(&it("Users", Kind::Table), Dialect::MySql), "Users");
        assert_eq!(insert_text(&it("order", Kind::Table), Dialect::MySql), "`order`");
        assert_eq!(insert_text(&it("my col", Kind::Column), Dialect::SqlServer), "[my col]");
        assert_eq!(insert_text(&it("SELECT", Kind::Keyword), Dialect::MySql), "SELECT");
    }
}

