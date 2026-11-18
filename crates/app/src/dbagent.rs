//! The chat's database tools: the AI sees DbCraft's connections, reads structure and data, writes
//! queries and opens them in the editor. As in OpenSlate's board agent, a change (INSERT, UPDATE,
//! DDL…) is never done straight away: the tool gives a code, the AI asks the user and calls again
//! with the code only after the user's yes, in a later message.
//!
//! The tools run on the chat's worker thread; the database work goes to DbCraft's tokio runtime
//! (`set_runtime`), with drivers of their own (the app's tabs are not disturbed).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use db_core::split::split_statements;
use db_core::{ConnectionConfig, Dialect, Driver, ObjectKind, ResultSet, Value};
use serde_json::{Value as Json, json};

/// What the AI may do with the databases (`[db]` of ai.toml).
#[derive(Clone, Debug)]
pub struct DbAccess {
    pub allow_write: bool,
    pub max_rows: usize,
}

/// The result of a tool, for the model and for the chat.
#[derive(Default)]
pub struct Outcome {
    /// For the model.
    pub text: String,
    /// One line for the chat («12 righe», «conferma chiesta»…).
    pub short: String,
    /// A query opened in the editor (its title).
    pub opened: Option<String>,
}

fn out(text: impl Into<String>, short: impl Into<String>) -> Outcome {
    Outcome { text: text.into(), short: short.into(), opened: None }
}

// ----- what the app shares with the chat -----

static RUNTIME: OnceLock<tokio::runtime::Handle> = OnceLock::new();
static CONNS: Mutex<Vec<ConnectionConfig>> = Mutex::new(Vec::new());
type DriverCache = HashMap<(String, String), Arc<dyn Driver>>;
static DRIVERS: Mutex<Option<DriverCache>> = Mutex::new(None);
/// A query to open in the editor: (connection id, database, schema, sql, title).
pub type ToOpen = (String, String, Option<String>, String, String);
/// Queries the AI asked to open in the editor.
static TO_OPEN: Mutex<Vec<ToOpen>> = Mutex::new(Vec::new());

/// DbCraft's runtime, for the database work of the tools.
pub fn set_runtime(h: tokio::runtime::Handle) {
    let _ = RUNTIME.set(h);
}

/// The configured connections (the app calls it when they change).
pub fn set_connections(configs: &[ConnectionConfig]) {
    let Ok(mut g) = CONNS.lock() else { return };
    let same = g.len() == configs.len() && g.iter().zip(configs).all(|(a, b)| a.id == b.id && a.name == b.name && a.host == b.host && a.database == b.database && a.user == b.user && a.port == b.port);
    if !same {
        *g = configs.to_vec();
        if let Ok(mut d) = DRIVERS.lock() {
            *d = None;
        }
    }
}

/// Queries to open in the editor (the app takes them each frame).
pub fn take_to_open() -> Vec<ToOpen> {
    TO_OPEN.lock().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
}

// ----- confirmations (as in OpenSlate's board agent) -----

/// Each question in the chat is a «turn»: a confirmation asked in a turn holds from the next one.
static TURN: Mutex<u64> = Mutex::new(0);
/// Changes waiting for the user's yes: (code, signature of connection + SQL, turn asked).
static PENDING: Mutex<Vec<(String, u64, u64)>> = Mutex::new(Vec::new());

/// SQL the user approved: shown by the AI in its last answer, and the user answered yes.
static APPROVED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The same SQL, whatever the spaces, case and final `;`.
fn norm_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(';').trim().to_lowercase()
}

/// A short yes, in the interface languages.
fn is_yes(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    let t = t.trim_matches(|c: char| !c.is_alphanumeric() && !c.is_whitespace());
    if t.chars().count() > 80 || t.starts_with("no") || t.contains(" non ") || t.starts_with("non ") {
        return false;
    }
    const YES: [&str; 26] = [
        "sì", "si", "yes", "ok", "okay", "procedi", "vai", "conferm", "esegui", "va bene", "d'accordo", "certo", "go", "sure", "do it", "oui", "ja", "sí", "да", "так", "是", "好", "はい", "네", "예", "confirm",
    ];
    YES.iter().any(|y| t == *y || t.starts_with(&format!("{y} ")) || t.starts_with(&format!("{y},")) || t.starts_with(y))
}

/// The SQL blocks of a Markdown text.
fn sql_blocks(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = md;
    while let Some(i) = rest.find("```") {
        let after = &rest[i + 3..];
        let Some(nl) = after.find('\n') else { break };
        let lang = after[..nl].trim().to_lowercase();
        let body = &after[nl + 1..];
        let Some(end) = body.find("```") else { break };
        if lang.is_empty() || lang.contains("sql") {
            out.push(body[..end].to_string());
        }
        rest = &body[end + 3..];
    }
    out
}

/// A new question in the chat: `shown` = the AI's previous answer, `answer` = the user's
/// message now. If the user said yes, the SQL shown there is approved.
pub fn new_turn(shown: &str, answer: &str) {
    if let Ok(mut t) = TURN.lock() {
        *t += 1;
    }
    if let Ok(mut a) = APPROVED.lock() {
        a.clear();
        if is_yes(answer) {
            for b in sql_blocks(shown) {
                a.push(norm_sql(&b));
                // and each statement of the block alone
                for st in split_statements(&b, Dialect::Postgres) {
                    a.push(norm_sql(&st));
                }
            }
        }
    }
}

fn approved(sql: &str) -> bool {
    let n = norm_sql(sql);
    APPROVED.lock().is_ok_and(|a| a.contains(&n))
}

fn turn() -> u64 {
    TURN.lock().map(|t| *t).unwrap_or(0)
}

fn signature(conn: &str, db: &str, sql: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (conn, db, sql.trim()).hash(&mut h);
    h.finish()
}

/// A short code for the confirmation (letters that are not confused).
fn confirm_code(seed: u64) -> String {
    const AL: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    (0..5)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            AL[(x >> 33) as usize % AL.len()] as char
        })
        .collect()
}

// ----- tools -----

fn s(args: &Json, k: &str) -> String {
    args[k].as_str().unwrap_or("").trim().to_string()
}

/// Definitions for the model (OpenAI "tools").
pub fn definitions(access: &DbAccess) -> Vec<Json> {
    let conn = json!({ "type": "string", "description": "Nome della connessione come in db_connections (vuoto = quella selezionata in DbCraft)" });
    let db = json!({ "type": "string", "description": "Database (vuoto = quello selezionato o il predefinito della connessione)" });
    let schema = json!({ "type": "string", "description": "Schema (PostgreSQL / SQL Server; vuoto = quello selezionato o public/dbo)" });
    let f = |name: &str, desc: &str, props: Json, req: &[&str]| {
        json!({ "type": "function", "function": { "name": name, "description": desc, "parameters": { "type": "object", "properties": props, "required": req } } })
    };
    let mut v = vec![
        f("db_connections", "Elenca le connessioni configurate in DbCraft (nome, tipo di database, server, database predefinito) e dice quale è selezionata.", json!({}), &[]),
        f("db_databases", "Elenca i database (e gli schemi) di una connessione.", json!({ "connection": conn }), &[]),
        f(
            "db_tables",
            "Elenca tabelle, viste, funzioni, procedure e trigger di un database o schema, con righe stimate e commenti.",
            json!({ "connection": conn, "database": db, "schema": schema }),
            &[],
        ),
        f(
            "db_describe",
            "La struttura di una tabella: colonne (tipo, null, predefinito, chiave primaria, auto incremento, commento), indici e chiavi esterne.",
            json!({ "connection": conn, "database": db, "schema": schema, "table": { "type": "string", "description": "Nome della tabella o vista" } }),
            &["table"],
        ),
        f(
            "db_query",
            "Esegue SQL di sola lettura (SELECT, WITH, SHOW, DESCRIBE, EXPLAIN, PRAGMA di lettura) e restituisce le righe come tabella. Per vedere i dati, contare, controllare un risultato. Le modifiche non passano da qui.",
            json!({ "connection": conn, "database": db, "sql": { "type": "string", "description": "Una o più istruzioni di lettura, nel dialetto del database" } }),
            &["sql"],
        ),
        f(
            "db_open_query",
            "Apre una nuova scheda Query nell'editor di DbCraft con questo SQL (non lo esegue): l'utente lo vede, lo modifica e lo esegue lui. Usalo quando l'utente vuole la query nell'editor o quando prepari una query da salvare o eseguire.",
            json!({ "connection": conn, "database": db, "schema": schema, "sql": { "type": "string" }, "title": { "type": "string", "description": "Titolo breve della scheda" } }),
            &["sql"],
        ),
    ];
    if access.allow_write {
        v.push(f(
            "db_execute",
            "Esegue SQL che modifica dati o struttura (INSERT, UPDATE, DELETE, CREATE, ALTER, DROP…). Chiamalo appena serve una modifica, prima di chiedere all'utente: la prima chiamata non esegue niente e dà un codice di conferma. Poi mostra all'utente l'SQL esatto, chiedigli se va bene e, solo dopo il suo sì nel messaggio successivo, richiama con le stesse istruzioni e «conferma».",
            json!({ "connection": conn, "database": db, "sql": { "type": "string" }, "conferma": { "type": "string", "description": "Il codice dato dallo strumento, dopo il sì dell'utente" } }),
            &["sql"],
        ));
    }
    v
}

/// What the model is told about the tools.
pub fn system_note(access: &DbAccess) -> String {
    let mut n = String::from(
        "\n- db_connections, db_databases, db_tables, db_describe: le connessioni di DbCraft e la struttura dei database. Guardale prima di scrivere query su tabelle che non conosci.\
         \n- db_query: legge i dati (solo SELECT e simili), al massimo ",
    );
    n.push_str(&format!("{} righe per risultato. Per tabelle grandi usa LIMIT/TOP, COUNT e aggregazioni.", access.max_rows));
    n.push_str("\n- db_open_query: mette una query in una nuova scheda dell'editor di DbCraft, senza eseguirla.");
    if access.allow_write {
        n.push_str(
            "\n- Se nel messaggio precedente hai mostrato l'SQL e l'utente ha detto di sì, db_execute con quello stesso SQL lo esegue subito.\
             \n- db_execute: modifiche ai dati o alla struttura. Non si fanno mai senza il sì dell'utente. Quando serve una modifica, chiama SUBITO db_execute senza «conferma»: \
             non esegue niente e ti dà un codice. Poi mostra all'utente l'SQL e chiedi se va bene. Solo se nel messaggio dopo risponde di sì, richiama con le stesse istruzioni e «conferma». \
             Non chiedere il sì prima di aver chiamato db_execute. Preferisci modifiche mirate (WHERE sulla chiave primaria).",
        );
    } else {
        n.push_str("\n- Modificare i database non è permesso in Impostazioni AI: se serve una modifica, scrivi l'SQL (o aprilo con db_open_query) e lo esegue l'utente.");
    }
    n.push_str("\nScrivi l'SQL nel dialetto del database (MySQL/MariaDB, PostgreSQL, SQL Server o SQLite: lo dice db_connections).");
    n
}

/// What the chat shows while a tool runs (Italian: translated by the page).
pub fn label(name: &str, args: &Json) -> String {
    let conn = s(args, "connection");
    let on = |what: &str| if conn.is_empty() { what.to_string() } else { format!("{what} ({conn})") };
    match name {
        "db_connections" => "Guardo le connessioni".into(),
        "db_databases" => on("Elenco i database"),
        "db_tables" => {
            let d = s(args, "database");
            if d.is_empty() { on("Elenco le tabelle") } else { format!("Elenco le tabelle di {d}") }
        }
        "db_describe" => format!("Leggo la struttura di {}", s(args, "table")),
        "db_query" => on("Eseguo una query"),
        "db_execute" if args["conferma"].as_str().is_some_and(|c| !c.trim().is_empty()) || approved(&s(args, "sql")) => on("Eseguo la modifica"),
        "db_execute" => on("Preparo la modifica"),
        "db_open_query" => "Apro la query nell'editor".into(),
        other => other.to_string(),
    }
}

/// The connection named by the model (name or id; empty = the one selected in DbCraft).
fn find_conn(name: &str) -> Result<ConnectionConfig, String> {
    let conns = CONNS.lock().map(|g| g.clone()).unwrap_or_default();
    if conns.is_empty() {
        return Err("in DbCraft non c'è nessuna connessione configurata".into());
    }
    let name = name.trim();
    if name.is_empty() {
        if let Some(c) = crate::ai::context().and_then(|c| conns.iter().find(|x| x.id == c.conn).cloned()) {
            return Ok(c);
        }
        if conns.len() == 1 {
            return Ok(conns[0].clone());
        }
        return Err(format!("nessuna connessione selezionata: indica quale fra {}", conns.iter().map(|c| format!("«{}»", c.name)).collect::<Vec<_>>().join(", ")));
    }
    conns
        .iter()
        .find(|c| c.name == name || c.id == name)
        .or_else(|| conns.iter().find(|c| c.name.eq_ignore_ascii_case(name)))
        .cloned()
        .ok_or_else(|| format!("connessione «{name}» non trovata: ci sono {}", conns.iter().map(|c| format!("«{}»", c.name)).collect::<Vec<_>>().join(", ")))
}

/// The database to use: the one asked, else the selected one (same connection), else the default.
fn pick_db(cfg: &ConnectionConfig, asked: &str) -> String {
    if !asked.is_empty() {
        return asked.to_string();
    }
    match crate::ai::context().filter(|c| c.conn == cfg.id) {
        Some(c) if !c.db.is_empty() => c.db,
        _ => cfg.database.clone(),
    }
}

fn pick_schema(cfg: &ConnectionConfig, db: &str, asked: &str) -> Option<String> {
    if !asked.is_empty() {
        return Some(asked.to_string());
    }
    match crate::ai::context().filter(|c| c.conn == cfg.id && c.db == db) {
        Some(c) => c.schema,
        None => match Dialect::from(cfg.kind) {
            Dialect::Postgres => Some("public".into()),
            Dialect::SqlServer => Some("dbo".into()),
            _ => None,
        },
    }
}

/// Runs `f` with a driver for (connection, database) on DbCraft's runtime.
fn with_driver<T>(cfg: &ConnectionConfig, db: &str, f: impl AsyncFnOnce(Arc<dyn Driver>) -> Result<T, String>) -> Result<T, String> {
    let rt = RUNTIME.get().ok_or("DbCraft non è pronto")?.clone();
    let key = (cfg.id.clone(), db.to_string());
    let cached = DRIVERS.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(&key).cloned()));
    let cfg = cfg.clone();
    rt.block_on(async move {
        let d = match cached {
            Some(d) => d,
            None => {
                let d = db_drivers::connect(&cfg, (!key.1.is_empty()).then_some(key.1.as_str())).await.map_err(|e| e.to_string())?;
                if let Ok(mut g) = DRIVERS.lock() {
                    g.get_or_insert_with(HashMap::new).insert(key, d.clone());
                }
                d
            }
        };
        f(d).await
    })
}

/// Read-only SQL? Every statement must start with a reading keyword (and not write as a side
/// effect: SELECT … INTO, EXPLAIN ANALYZE of a change, PRAGMA x = y).
pub fn read_only(sql: &str, d: Dialect) -> bool {
    let stmts = split_statements(sql, d);
    !stmts.is_empty()
        && stmts.iter().all(|st| {
            let words: Vec<String> = st
                .split(|c: char| c.is_whitespace() || c == '(' || c == ';')
                .filter(|w| !w.is_empty())
                .map(|w| w.to_ascii_uppercase())
                .collect();
            let first = words.first().map(String::as_str).unwrap_or("");
            let writes = ["INSERT", "UPDATE", "DELETE", "MERGE", "CREATE", "ALTER", "DROP", "TRUNCATE", "GRANT", "REVOKE", "REPLACE", "CALL", "EXEC", "EXECUTE"];
            let has_write_word = words.iter().any(|w| writes.contains(&w.as_str()));
            match first {
                "SELECT" | "VALUES" | "TABLE" => !words.iter().any(|w| w == "INTO") && !has_write_word,
                "WITH" => !has_write_word,
                "SHOW" | "DESCRIBE" | "DESC" => true,
                "EXPLAIN" => !words.iter().any(|w| w == "ANALYZE") || !has_write_word,
                "PRAGMA" => !st.contains('='),
                _ => false,
            }
        })
}

fn cell(v: &Value) -> String {
    let t = match v {
        Value::Null => "NULL".to_string(),
        Value::Bytes(b) if std::str::from_utf8(b).is_ok() => String::from_utf8_lossy(b).into_owned(),
        other => other.to_string(),
    };
    let t = t.replace('|', "\\|").replace('\n', " ");
    if t.chars().count() > 200 { format!("{}…", t.chars().take(200).collect::<String>()) } else { t }
}

/// A result set as a Markdown table (at most `max` rows).
fn table_md(rs: &ResultSet, max: usize) -> String {
    if rs.columns.is_empty() {
        return match rs.affected {
            Some(n) => format!("{n} righe interessate"),
            None => "fatto (nessun risultato)".into(),
        };
    }
    let mut t = format!("| {} |\n|{}\n", rs.columns.iter().map(|c| c.name.replace('|', "\\|")).collect::<Vec<_>>().join(" | "), "---|".repeat(rs.columns.len()));
    for row in rs.rows.iter().take(max) {
        t.push_str(&format!("| {} |\n", row.iter().map(cell).collect::<Vec<_>>().join(" | ")));
    }
    if rs.rows.len() > max {
        t.push_str(&format!("\n[… altre {} righe non mostrate: restringi con WHERE, LIMIT o aggregazioni]\n", rs.rows.len() - max));
    }
    t
}

/// Runs a tool asked by the model.
pub fn execute(access: &DbAccess, name: &str, args: &Json) -> Outcome {
    match run(access, name, args) {
        Ok(o) => o,
        Err(e) => out(format!("ERRORE: {e}"), format!("errore: {e}")),
    }
}

fn run(access: &DbAccess, name: &str, args: &Json) -> Result<Outcome, String> {
    match name {
        "db_connections" => {
            let conns = CONNS.lock().map(|g| g.clone()).unwrap_or_default();
            let sel = crate::ai::context();
            let mut t = String::new();
            for c in &conns {
                let server = if c.kind == db_core::DbKind::Sqlite { c.database.clone() } else { format!("{}:{}", c.host, c.port) };
                let selected = sel.as_ref().filter(|s| s.conn == c.id).map(|s| format!(" — SELEZIONATA in DbCraft: database «{}»{}", s.db, s.schema.as_deref().map(|x| format!(", schema «{x}»")).unwrap_or_default())).unwrap_or_default();
                t.push_str(&format!("- «{}»: {} su {}, database predefinito «{}»{selected}\n", c.name, c.kind.label(), server, c.database));
            }
            if t.is_empty() {
                t = "Nessuna connessione configurata in DbCraft.".into();
            }
            Ok(out(t, format!("{} connessioni", conns.len())))
        }
        "db_databases" => {
            let cfg = find_conn(&s(args, "connection"))?;
            let dbs = with_driver(&cfg, "", async |d| d.list_databases().await.map_err(|e| e.to_string()))?;
            let mut t = format!("Database di «{}» ({}):\n", cfg.name, cfg.kind.label());
            for db in &dbs {
                t.push_str(&format!("- {db}\n"));
            }
            if matches!(Dialect::from(cfg.kind), Dialect::Postgres | Dialect::SqlServer) {
                let db = pick_db(&cfg, "");
                if let Ok(schemas) = with_driver(&cfg, &db, async |d| d.list_schemas().await.map_err(|e| e.to_string())) {
                    t.push_str(&format!("\nSchemi di «{db}»: {}\n", schemas.join(", ")));
                }
            }
            Ok(out(t, format!("{} database", dbs.len())))
        }
        "db_tables" => {
            let cfg = find_conn(&s(args, "connection"))?;
            let db = pick_db(&cfg, &s(args, "database"));
            let schema = pick_schema(&cfg, &db, &s(args, "schema"));
            let sc = schema.clone();
            let objs = with_driver(&cfg, &db, async move |d| d.list_objects(sc.as_deref()).await.map_err(|e| e.to_string()))?;
            let mut t = format!("Oggetti di «{db}»{} su «{}» ({}):\n", schema.as_deref().map(|x| format!(", schema «{x}»")).unwrap_or_default(), cfg.name, cfg.kind.label());
            for kind in [ObjectKind::Table, ObjectKind::View, ObjectKind::Function, ObjectKind::Procedure, ObjectKind::Trigger] {
                let items: Vec<String> = objs
                    .iter()
                    .filter(|o| o.kind == kind)
                    .map(|o| {
                        let mut x = o.name.clone();
                        if let Some(r) = o.rows {
                            x.push_str(&format!(" (~{r} righe)"));
                        }
                        if let Some(c) = o.comment.as_deref().filter(|c| !c.is_empty()) {
                            x.push_str(&format!(" — {c}"));
                        }
                        x
                    })
                    .collect();
                if !items.is_empty() {
                    t.push_str(&format!("\n{}:\n- {}\n", kind.label(), items.join("\n- ")));
                }
            }
            let n = objs.iter().filter(|o| o.kind == ObjectKind::Table).count();
            Ok(out(t, format!("{n} tabelle")))
        }
        "db_describe" => {
            let cfg = find_conn(&s(args, "connection"))?;
            let db = pick_db(&cfg, &s(args, "database"));
            let schema = pick_schema(&cfg, &db, &s(args, "schema"));
            let table = s(args, "table");
            if table.is_empty() {
                return Err("manca «table»".into());
            }
            let (sc, tb) = (schema.clone(), table.clone());
            let info = with_driver(&cfg, &db, async move |d| d.describe_table(sc.as_deref(), &tb).await.map_err(|e| e.to_string()))?;
            let mut t = format!("Tabella «{table}» di «{db}» ({}):\n\n| colonna | tipo | null | predefinito | chiave | note |\n|---|---|---|---|---|---|\n", cfg.kind.label());
            for c in &info.columns {
                t.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {}{} |\n",
                    c.name,
                    c.data_type,
                    if c.nullable { "sì" } else { "no" },
                    c.default.clone().unwrap_or_default(),
                    if c.primary_key { "PK" } else { "" },
                    if c.auto_increment { "auto incremento " } else { "" },
                    c.comment.clone().unwrap_or_default()
                ));
            }
            if !info.indexes.is_empty() {
                t.push_str("\nIndici:\n");
                for i in &info.indexes {
                    t.push_str(&format!("- {} ({}){}\n", i.name, i.columns.join(", "), if i.unique { " UNIQUE" } else { "" }));
                }
            }
            if !info.foreign_keys.is_empty() {
                t.push_str("\nChiavi esterne:\n");
                for f in &info.foreign_keys {
                    t.push_str(&format!("- ({}) → {}({})\n", f.columns.join(", "), f.ref_table, f.ref_columns.join(", ")));
                }
            }
            Ok(out(t, format!("{} colonne", info.columns.len())))
        }
        "db_query" => {
            let cfg = find_conn(&s(args, "connection"))?;
            let db = pick_db(&cfg, &s(args, "database"));
            let sql = s(args, "sql");
            if sql.is_empty() {
                return Err("manca «sql»".into());
            }
            if !read_only(&sql, Dialect::from(cfg.kind)) {
                return Ok(out(
                    if access.allow_write {
                        "NON FATTO: db_query esegue solo letture. Per modificare usa db_execute (che chiede la conferma dell'utente)."
                    } else {
                        "NON FATTO: db_query esegue solo letture e le modifiche non sono permesse in Impostazioni AI: scrivi l'SQL per l'utente."
                    },
                    "solo lettura",
                ));
            }
            let started = std::time::Instant::now();
            let res = with_driver(&cfg, &db, async move |d| d.execute(&sql).await.map_err(|e| e.to_string()))?;
            let mut t = String::new();
            let mut rows = 0;
            for (i, rs) in res.iter().enumerate() {
                if res.len() > 1 {
                    t.push_str(&format!("\nRisultato {}:\n", i + 1));
                }
                rows += rs.rows.len();
                t.push_str(&format!("{} righe\n\n{}\n", rs.rows.len(), table_md(rs, access.max_rows)));
            }
            t.push_str(&format!("\n(tempo: {:.3}s)", started.elapsed().as_secs_f64()));
            Ok(out(t, format!("{rows} righe")))
        }
        "db_execute" => {
            if !access.allow_write {
                return Ok(out("NON FATTO: le modifiche ai database non sono permesse in Impostazioni AI. Scrivi l'SQL per l'utente.", "non permesso"));
            }
            let cfg = find_conn(&s(args, "connection"))?;
            let db = pick_db(&cfg, &s(args, "database"));
            let sql = s(args, "sql");
            if sql.is_empty() {
                return Err("manca «sql»".into());
            }
            let sig = signature(&cfg.id, &db, &sql);
            let given = s(args, "conferma");
            let now = turn();
            let mut pending = PENDING.lock().map_err(|e| e.to_string())?;
            let found = pending.iter().find(|(_, s, _)| *s == sig).cloned();
            // the user saw this very SQL in the last answer and said yes
            let found = if approved(&sql) { Some((given.clone(), sig, 0)) } else { found };
            match found {
                Some((code, _, asked)) if (!given.is_empty() && given.eq_ignore_ascii_case(&code) && asked < now) || approved(&sql) => {
                    pending.retain(|(_, s, _)| *s != sig);
                    drop(pending);
                    let res = with_driver(&cfg, &db, async move |d| d.execute(&sql).await.map_err(|e| e.to_string()))?;
                    let affected: u64 = res.iter().filter_map(|r| r.affected).sum();
                    let how = if given.is_empty() { " (è l'SQL che l'utente ha approvato con il suo sì: niente codice da chiedere)" } else { "" };
                    let t = format!("Fatto{how}: {} istruzioni eseguite su «{}», {affected} righe interessate.", res.len(), db);
                    Ok(out(t, format!("{affected} righe modificate")))
                }
                Some((_, _, asked)) if !given.is_empty() && asked >= now => Ok(out(
                    "NON FATTO: la conferma la dà l'utente. Mostragli l'SQL, chiedigli se va bene e aspetta la sua risposta prima di richiamare.",
                    "conferma chiesta",
                )),
                _ => {
                    let code = confirm_code(sig ^ now);
                    pending.retain(|(_, s, _)| *s != sig);
                    pending.push((code.clone(), sig, now));
                    Ok(out(
                        format!(
                            "NON ANCORA FATTO: serve il sì dell'utente. Mostragli l'SQL esatto (in un blocco ```sql) e dove verrà eseguito (connessione «{}», database «{db}»), \
                             chiedigli se va bene e fermati. Se nel suo prossimo messaggio dice di sì, richiama db_execute con le stesse istruzioni e \"conferma\": \"{code}\".",
                            cfg.name
                        ),
                        "conferma chiesta",
                    ))
                }
            }
        }
        "db_open_query" => {
            let cfg = find_conn(&s(args, "connection"))?;
            let db = pick_db(&cfg, &s(args, "database"));
            let schema = pick_schema(&cfg, &db, &s(args, "schema"));
            let sql = s(args, "sql");
            if sql.is_empty() {
                return Err("manca «sql»".into());
            }
            let title = Some(s(args, "title")).filter(|t| !t.is_empty()).unwrap_or_else(|| "Query AI".into());
            if let Ok(mut g) = TO_OPEN.lock() {
                g.push((cfg.id.clone(), db.clone(), schema, sql, title.clone()));
            }
            if let Some(c) = crate::web::wake() {
                c.request_repaint();
            }
            Ok(Outcome { text: format!("Aperta in DbCraft la scheda «{title}» (connessione «{}», database «{db}»): l'utente può eseguirla con ⌘/Ctrl+Invio.", cfg.name), short: title.clone(), opened: Some(title) })
        }
        other => Err(format!("strumento sconosciuto: {other}")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn only_reading_passes() {
        let d = Dialect::Postgres;
        assert!(read_only("SELECT * FROM t WHERE x = 'DELETE'", d));
        assert!(read_only("select a, count(*) from t group by a; show tables", Dialect::MySql));
        assert!(read_only("WITH x AS (SELECT 1) SELECT * FROM x", d));
        assert!(read_only("EXPLAIN SELECT 1", d));
        assert!(read_only("PRAGMA table_info(t)", Dialect::Sqlite));
        assert!(!read_only("PRAGMA foreign_keys = OFF", Dialect::Sqlite));
        assert!(!read_only("DELETE FROM t", d));
        assert!(!read_only("SELECT 1; DROP TABLE t", d));
        assert!(!read_only("SELECT * INTO copy FROM t", d));
        assert!(!read_only("WITH x AS (DELETE FROM t RETURNING *) SELECT * FROM x", d));
        assert!(!read_only("EXPLAIN ANALYZE DELETE FROM t", d));
        assert!(!read_only("", d));
    }

    #[test]
    fn approval_by_yes() {
        let shown = "Ecco:\n\n```sql\nUPDATE customers SET credit = credit + 10\n  WHERE id = 4;\n```\n\nProcedo?";
        new_turn(shown, "Sì, procedi");
        assert!(approved("update customers set credit = credit + 10 where id = 4"));
        assert!(!approved("DELETE FROM customers"));
        new_turn(shown, "no, aspetta");
        assert!(!approved("UPDATE customers SET credit = credit + 10 WHERE id = 4"));
        assert!(is_yes("ok") && is_yes("Yes, go ahead") && is_yes("はい") && !is_yes("non ancora") && !is_yes("no"));
    }

    #[test]
    fn markdown_table() {
        let rs = ResultSet {
            columns: vec![db_core::ColumnMeta { name: "a".into(), type_name: String::new() }, db_core::ColumnMeta { name: "b".into(), type_name: String::new() }],
            rows: vec![vec![Value::Int(1), Value::Text("x|y".into())], vec![Value::Null, Value::Text("z".into())]],
            ..Default::default()
        };
        let t = table_md(&rs, 1);
        assert!(t.starts_with("| a | b |\n|---|---|\n| 1 | x\\|y |"));
        assert!(t.contains("altre 1 righe"));
    }
}
