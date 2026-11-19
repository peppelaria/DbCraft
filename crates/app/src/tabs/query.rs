//! Query editor: SQL with syntax highlighting and code completion; run all, the selection or
//! the statement at the cursor; stop; explain; beautify; snippets; one result tab per statement
//! plus a message log.

use crate::icons::{self, Db, G};
use std::time::Instant;

use db_core::editable::{SelectSource, select_source};
use db_core::split::statement_at;
use db_core::{DbKind, Dialect, ResultSet, RowChange, TableInfo, Value};
use dbgrid::{ColumnKind, DataGrid, GridColumn};
use egui::text::{CCursor, CCursorRange};
use egui::{Key, Modifiers, RichText};

use crate::app::{Action, Env, Location};
use crate::backend::{Msg, TabId};
use crate::dialogs::ExportSource;
use crate::models::{Edits, ResultModel, grid_columns};
use crate::tabs::builder::{BuilderRequest, QueryBuilder};
use crate::tabs::complete::{self, Catalog, Item, Kind};
use crate::i18n;

struct ResultView {
    label: String,
    rs: ResultSet,
    columns: Vec<GridColumn>,
    grid: DataGrid,
    /// Set when the cells can be edited (see [`ResultEdit`]).
    edit: Option<ResultEdit>,
    edits: Edits,
}

/// An editable result: the rows of a single-table `SELECT` that includes the table's key.
/// Edits become `UPDATE table SET <changed columns> WHERE <key> = <value of that row>`, one per
/// row, shown before they run and applied in one transaction that fails if any of them touches
/// more than one row.
struct ResultEdit {
    schema: Option<String>,
    table: String,
    /// Result column → table column, for the key.
    key: Vec<(usize, String)>,
    /// Result column → table column, for every editable column (`None`: read-only).
    cols: Vec<Option<String>>,
    /// The same, as the grid wants it.
    editable: Vec<bool>,
}

/// What the query tab knows about whether its result can be edited.
enum Editing {
    /// Not a single-table `SELECT` (or nothing ran yet).
    No,
    /// Reading the table's columns and key.
    Checking,
    Yes(String),
    /// A single-table `SELECT` that can't be edited, and why.
    ReadOnly(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Run {
    All,
    Selection,
    Current,
    Explain,
}

enum Meta {
    Idle,
    Loading,
    Ready(String),
    Failed(String),
}

struct Popup {
    items: Vec<Item>,
    sel: usize,
    /// Char index where the completed word starts.
    start: usize,
    forced: bool,
    /// Scroll the selected row into view.
    scroll: bool,
    /// Columns are being loaded.
    loading: bool,
    rect: egui::Rect,
}

pub struct QueryTab {
    pub id: TabId,
    pub loc: Location,
    sql: String,
    results: Vec<ResultView>,
    /// 0 = messages, i = results[i - 1].
    active_result: usize,
    messages: Vec<(String, bool)>,
    running: Option<Instant>,
    /// Identifies the current run: results of a stopped one are ignored.
    run_id: u64,
    job: Option<tokio::task::AbortHandle>,
    /// The text sent to the server (to cancel it there).
    running_sql: String,
    explain: bool,
    last: String,
    pub path: Option<std::path::PathBuf>,
    pending_run: bool,
    /// The running script changes the schema: refresh the tree when it finishes.
    ddl: bool,
    /// The first result belongs to the `SET search_path` prefix.
    skip_first: bool,
    builder: Option<QueryBuilder>,
    show_builder: bool,
    // Editor state.
    cursor: usize,
    selection: Option<(usize, usize)>,
    popup: Option<Popup>,
    /// Put the cursor here (char index) on the next frame.
    move_cursor: Option<usize>,
    // Code completion and the location pickers.
    meta: Meta,
    meta_gen: u64,
    catalog: Catalog,
    dbs: Vec<String>,
    schemas: Vec<String>,
    complete_when_ready: bool,
    /// The table the running `SELECT` reads (for editing its result).
    source: Option<SelectSource>,
    editing: Editing,
    saving: bool,
    /// The statements of "Save", waiting for confirmation.
    confirm: Option<Vec<String>>,
    /// Scripted (`--query … --table-ui edit`): edit the first editable cell and ask to save.
    pub debug_edit: bool,
    /// Scripted (`--query … --table-ui picker`): open the date picker of the first date column.
    pub debug_picker: bool,
}

fn changes_schema(sql: &str) -> bool {
    sql.split(';').any(|s| {
        let w = s.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
        matches!(w.as_str(), "CREATE" | "DROP" | "ALTER" | "RENAME" | "TRUNCATE" | "COMMENT")
    })
}

fn byte_of(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
}

fn char_of(s: &str, byte: usize) -> usize {
    s[..byte.min(s.len())].chars().count()
}

/// Ready-made statements for the Snippet menu.
fn snippets(d: Dialect) -> Vec<(&'static str, String)> {
    let page = match d {
        Dialect::SqlServer => "SELECT *\nFROM table_name\nORDER BY id\nOFFSET 0 ROWS FETCH NEXT 100 ROWS ONLY;",
        _ => "SELECT *\nFROM table_name\nORDER BY id\nLIMIT 100 OFFSET 0;",
    };
    let id = match d {
        Dialect::Postgres => "id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY",
        Dialect::MySql => "id bigint AUTO_INCREMENT PRIMARY KEY",
        Dialect::SqlServer => "id bigint IDENTITY(1,1) PRIMARY KEY",
        Dialect::Sqlite => "id INTEGER PRIMARY KEY AUTOINCREMENT",
    };
    let tx = match d {
        Dialect::SqlServer => "BEGIN TRANSACTION;\n\n-- …\n\nCOMMIT;",
        Dialect::MySql => "START TRANSACTION;\n\n-- …\n\nCOMMIT;",
        _ => "BEGIN;\n\n-- …\n\nCOMMIT;",
    };
    vec![
        ("SELECT", "SELECT *\nFROM table_name\nWHERE condition;".into()),
        ("SELECT con JOIN", "SELECT a.*, b.*\nFROM table_a a\nJOIN table_b b ON b.a_id = a.id\nWHERE condition;".into()),
        ("SELECT con GROUP BY", "SELECT column_name, COUNT(*) AS n\nFROM table_name\nGROUP BY column_name\nHAVING COUNT(*) > 1\nORDER BY n DESC;".into()),
        ("Paginazione", page.into()),
        ("WITH (CTE)", "WITH cte AS (\n  SELECT *\n  FROM table_name\n)\nSELECT *\nFROM cte;".into()),
        ("CASE", "CASE\n  WHEN condition THEN result\n  ELSE other_result\nEND".into()),
        ("INSERT", "INSERT INTO table_name (column1, column2)\nVALUES (value1, value2);".into()),
        ("UPDATE", "UPDATE table_name\nSET column1 = value1\nWHERE condition;".into()),
        ("DELETE", "DELETE FROM table_name\nWHERE condition;".into()),
        ("CREATE TABLE", format!("CREATE TABLE table_name (\n  {id},\n  name varchar(100) NOT NULL,\n  created_at timestamp DEFAULT CURRENT_TIMESTAMP\n);")),
        ("CREATE VIEW", "CREATE VIEW view_name AS\nSELECT *\nFROM table_name;".into()),
        ("CREATE INDEX", "CREATE INDEX index_name ON table_name (column_name);".into()),
        ("ALTER TABLE ADD COLUMN", "ALTER TABLE table_name ADD column_name varchar(100);".into()),
        ("Transazione", tx.into()),
    ]
}

fn kind_icon(k: Kind) -> icons::Ico {
    match k {
        Kind::Column => Db::Column.into(),
        Kind::Table => Db::Table.into(),
        Kind::View => Db::View.into(),
        Kind::Function => Db::Function.into(),
        Kind::Procedure => Db::Procedure.into(),
        Kind::Keyword => Db::Keyword.into(),
    }
}

impl QueryTab {
    pub fn new(id: TabId, loc: Location) -> Self {
        QueryTab {
            id,
            loc,
            sql: String::new(),
            results: Vec::new(),
            active_result: 0,
            messages: Vec::new(),
            running: None,
            run_id: 0,
            job: None,
            running_sql: String::new(),
            explain: false,
            last: String::new(),
            path: None,
            pending_run: false,
            ddl: false,
            skip_first: false,
            builder: None,
            show_builder: false,
            cursor: 0,
            selection: None,
            popup: None,
            move_cursor: None,
            meta: Meta::Idle,
            meta_gen: 0,
            catalog: Catalog::default(),
            dbs: Vec::new(),
            schemas: Vec::new(),
            complete_when_ready: false,
            source: None,
            editing: Editing::No,
            saving: false,
            confirm: None,
            debug_edit: false,
            debug_picker: false,
        }
    }

    pub fn set_sql(&mut self, sql: String, run: bool) {
        self.sql = sql;
        self.pending_run = run;
    }

    fn dialect(&self, env: &Env<'_>) -> Dialect {
        env.dialect(&self.loc.conn)
    }

    /// The text `what` refers to.
    fn text_for(&self, what: Run, d: Dialect) -> String {
        let selected = self.selection.map(|(a, b)| self.sql.chars().skip(a).take(b - a).collect::<String>()).filter(|s| !s.trim().is_empty());
        let current = || statement_at(&self.sql, d, byte_of(&self.sql, self.cursor)).map(|r| self.sql[r].to_string()).unwrap_or_default();
        match what {
            Run::All => self.sql.clone(),
            Run::Selection => selected.unwrap_or_else(|| self.sql.clone()),
            Run::Current | Run::Explain => selected.unwrap_or_else(current),
        }
    }

    fn run(&mut self, env: &mut Env<'_>, what: Run) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        let d = Dialect::from(cfg.kind);
        let sql = self.text_for(what, d);
        if sql.trim().is_empty() || self.running.is_some() {
            return;
        }
        if self.results.iter().any(|v| !v.edits.is_empty()) || self.saving {
            env.actions.push(Action::Notice("Salva o annulla le modifiche ai risultati prima di eseguire di nuovo".into(), true));
            return;
        }
        let explain = what == Run::Explain;
        self.source = if explain { None } else { select_source(&sql, d) };
        self.editing = Editing::No;
        self.confirm = None;
        self.running = Some(Instant::now());
        self.explain = explain;
        self.messages.push((format!("> {}{}", if explain { "EXPLAIN " } else { "" }, sql.trim()), false));
        self.ddl = !explain && changes_schema(&sql);
        // PostgreSQL: unqualified names resolve in the schema selected in the tree.
        let prefix = match (&self.loc.schema, cfg.kind) {
            (Some(s), DbKind::Postgres) if s != "public" => Some(format!("SET search_path TO {}, public;", Dialect::Postgres.quote(s))),
            _ => None,
        };
        self.skip_first = prefix.is_some();
        let sent = match (&prefix, explain) {
            (Some(p), true) => format!("{p}\nEXPLAIN {}", sql.trim().trim_end_matches(';')),
            (Some(p), false) => format!("{p}\n{sql}"),
            (None, _) => sql,
        };
        self.running_sql = sent.clone();
        self.run_id += 1;
        let (tab, run) = (self.id, self.run_id);
        let use_explain = explain && prefix.is_none();
        self.job = Some(env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) if use_explain => d.explain(&sent).await,
                Ok(d) => d.execute(&sent).await,
                Err(e) => Err(e),
            };
            Msg::QueryDone { tab, run, res }
        }));
    }

    /// Stops the running query: drops the client side, closes its connection, and on
    /// PostgreSQL/MySQL also cancels the statement on the server.
    fn stop(&mut self, env: &mut Env<'_>) {
        let Some(job) = self.job.take() else { return };
        job.abort();
        self.run_id += 1;
        let elapsed = self.running.take().map(|t| t.elapsed()).unwrap_or_default();
        self.messages.push((format!("Interrotta dall'utente dopo {:.3}s", elapsed.as_secs_f64()), true));
        self.last = "Interrotta".into();
        self.active_result = 0;
        self.ddl = false;
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        env.backend.evict(&cfg.id, &self.loc.db);
        let sent = std::mem::take(&mut self.running_sql);
        let cancel = db_core::dialect::cancel_query_sql(cfg.kind, &sent);
        let Some(cancel) = cancel else { return };
        let mysql = cfg.kind != DbKind::Postgres;
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = async {
                let d = d?;
                let rs = d.execute(&cancel).await?;
                if mysql {
                    for row in rs.first().map(|r| r.rows.clone()).unwrap_or_default() {
                        if let Some(id) = row.first().and_then(db_core::Value::as_i64) {
                            d.execute(&format!("KILL QUERY {id}")).await?;
                        }
                    }
                }
                Ok::<(), db_core::DbError>(())
            }
            .await;
            match res {
                Ok(()) => Msg::Notice { text: "Query interrotta".into(), error: false },
                Err(e) => Msg::Notice { text: format!("Interruzione sul server non riuscita: {e}"), error: true },
            }
        });
    }

    fn reset_meta(&mut self) {
        self.meta = Meta::Idle;
        self.meta_gen += 1;
        self.catalog = Catalog::default();
        self.popup = None;
    }

    fn request_meta(&mut self, env: &mut Env<'_>) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        self.meta = Meta::Loading;
        let (tab, generation, schema) = (self.id, self.meta_gen, self.loc.schema.clone());
        let want_schemas = matches!(cfg.kind, DbKind::Postgres | DbKind::SqlServer);
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            match d {
                Ok(d) => {
                    let dbs = Some(d.list_databases().await);
                    let schemas = if want_schemas { Some(d.list_schemas().await) } else { None };
                    let objects = d.list_objects(schema.as_deref()).await;
                    Msg::QueryMeta { tab, generation, dbs, schemas, objects }
                }
                Err(e) => Msg::QueryMeta { tab, generation, dbs: None, schemas: None, objects: Err(e) },
            }
        });
    }

    fn request_columns(&mut self, env: &mut Env<'_>, tables: Vec<String>) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        for lower in tables {
            if !self.catalog.requested.insert(lower.clone()) {
                continue;
            }
            let Some(name) = self.catalog.objects.iter().find(|o| o.name.to_lowercase() == lower).map(|o| o.name.clone()) else { continue };
            let (tab, generation, schema) = (self.id, self.meta_gen, self.loc.schema.clone());
            env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
                let res = match d {
                    Ok(d) => d.describe_table(schema.as_deref(), &name).await,
                    Err(e) => Err(e),
                };
                Msg::QueryColumns { tab, generation, table: lower, res }
            });
        }
    }

    pub fn handle(&mut self, msg: Msg, env: &mut Env<'_>) {
        let res = match msg {
            Msg::QueryDone { run, res, .. } => {
                if run != self.run_id {
                    return;
                }
                self.job = None;
                res
            }
            Msg::QueryMeta { generation, dbs, schemas, objects, .. } => {
                if generation != self.meta_gen {
                    return;
                }
                if let Some(Ok(v)) = dbs {
                    self.dbs = v;
                }
                if let Some(Ok(v)) = schemas {
                    self.schemas = v;
                }
                match objects {
                    Ok(objs) => {
                        self.catalog.objects = objs;
                        self.catalog.label = self.loc.schema.clone().filter(|_| self.loc.db.is_empty()).unwrap_or_else(|| self.loc.db.clone());
                        self.meta = Meta::Ready(chrono::Local::now().format("%H:%M").to_string());
                    }
                    Err(e) => self.meta = Meta::Failed(e.to_string()),
                }
                return;
            }
            Msg::QueryColumns { generation, table, res, .. } => {
                if generation != self.meta_gen {
                    return;
                }
                if res.is_err() && self.popup.as_ref().is_some_and(|p| p.loading) {
                    self.popup = None;
                }
                if let Ok(info) = res {
                    self.catalog.columns.insert(table, info.columns.into_iter().map(|c| (c.name, c.data_type)).collect());
                    // Refresh an open popup that was waiting for them.
                    if let Some(p) = &mut self.popup {
                        p.loading = false;
                        let forced = p.forced;
                        self.update_popup(env, forced, false);
                    }
                }
                return;
            }
            Msg::BuilderTables { res, .. } => {
                if let Some(b) = self.builder.as_mut() {
                    b.on_tables(res);
                }
                return;
            }
            Msg::ResultTable { run, res, .. } => {
                if run == self.run_id {
                    self.on_result_table(res);
                }
                return;
            }
            Msg::Applied { res, .. } => {
                self.on_applied(res, env);
                return;
            }
            Msg::BuilderTable { name, res, .. } => {
                if let Some(b) = self.builder.as_mut() {
                    b.on_table_info(&name, res);
                }
                return;
            }
            _ => return,
        };
        if std::mem::take(&mut self.ddl) {
            env.actions.push(Action::Refresh(self.loc.clone()));
            // New tables show up in completion too.
            self.reset_meta();
        }
        let skip = usize::from(std::mem::take(&mut self.skip_first));
        let elapsed = self.running.take().map(|t| t.elapsed()).unwrap_or_default();
        let explain = std::mem::take(&mut self.explain);
        match res {
            Ok(sets) => {
                let mut views = Vec::new();
                for rs in sets.into_iter().skip(skip) {
                    if rs.columns.is_empty() {
                        let n = rs.affected.map(|a| format!("{a} righe interessate")).unwrap_or_else(|| "OK".into());
                        self.messages.push((n.to_string(), false));
                    } else {
                        let truncated = rs.rows.len() >= 500_000;
                        self.messages.push((format!("{} righe restituite{}", rs.rows.len(), if truncated { " (troncate)" } else { "" }), false));
                        let columns = grid_columns(&rs, None);
                        let i = views.len();
                        let label = if explain { "Explain".to_string() } else { i18n::tf(format!("Risultato {}", i + 1)) };
                        views.push(ResultView { label, rs, columns, grid: DataGrid::new(("result", self.id, i, self.messages.len())), edit: None, edits: Edits::default() });
                    }
                }
                self.messages.push((format!("Tempo: {:.3}s", elapsed.as_secs_f64()), false));
                self.active_result = if views.is_empty() { 0 } else { 1 };
                self.last = format!("{} risultati — {:.3}s", views.len(), elapsed.as_secs_f64());
                // one result from a single-table SELECT: find out whether it can be edited
                let source = self.source.take().filter(|_| views.len() == 1);
                self.results = views;
                if let Some(src) = source {
                    self.check_editable(src, env);
                }
            }
            Err(e) => {
                self.messages.push((e.to_string(), true));
                self.messages.push((format!("Tempo: {:.3}s", elapsed.as_secs_f64()), false));
                self.active_result = 0;
                self.last = "Errore".into();
            }
        }
    }

    // ----- editing the result -----

    /// Reads the table a single-table `SELECT` comes from, to see whether its result can be edited.
    fn check_editable(&mut self, src: SelectSource, env: &mut Env<'_>) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        // the schema the table is in, as `describe_table` and the UPDATE want it
        let schema = match (Dialect::from(cfg.kind), src.schema.clone()) {
            (Dialect::MySql, Some(db)) if !db.eq_ignore_ascii_case(&self.loc.db) => {
                self.editing = Editing::ReadOnly(i18n::tf(format!("la tabella è in un altro database ({db})")));
                return;
            }
            (Dialect::Sqlite, Some(s)) if !s.eq_ignore_ascii_case("main") => {
                self.editing = Editing::ReadOnly(i18n::tf(format!("la tabella è in un altro database ({s})")));
                return;
            }
            (Dialect::MySql | Dialect::Sqlite, _) => None,
            (Dialect::Postgres, s) => s.or_else(|| self.loc.schema.clone()),
            (Dialect::SqlServer, s) => s,
        };
        self.editing = Editing::Checking;
        let (tab, run, table, sch) = (self.id, self.run_id, src.table.clone(), schema.clone());
        self.source = Some(SelectSource { schema, ..src });
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.describe_table(sch.as_deref(), &table).await,
                Err(e) => Err(e),
            };
            Msg::ResultTable { tab, run, res }
        });
    }

    /// The table's structure arrived: the result is editable if it holds the table's key (the
    /// primary key, or else a unique index on NOT NULL columns).
    fn on_result_table(&mut self, res: db_core::DbResult<TableInfo>) {
        let Some(src) = self.source.take() else { return };
        let Some(v) = self.results.first_mut() else { return };
        let info = match res {
            Ok(i) if !i.columns.is_empty() => i,
            Ok(_) => {
                self.editing = Editing::ReadOnly(i18n::tf(format!("tabella \"{}\" non trovata", src.table)));
                return;
            }
            Err(e) => {
                self.editing = Editing::ReadOnly(i18n::tf(format!("struttura della tabella non letta: {e}")));
                return;
            }
        };
        let table_cols: Vec<String> = info.columns.iter().map(|c| c.name.clone()).collect();
        let names: Vec<String> = v.rs.columns.iter().map(|c| c.name.clone()).collect();
        let mapped = src.map_columns(&names, &table_cols);
        let pk = info.primary_key();
        let not_null = |c: &String| info.columns.iter().any(|x| &x.name == c && !x.nullable);
        let unique = info.indexes.iter().filter(|i| i.unique && !i.columns.is_empty() && i.columns.iter().all(not_null)).map(|i| i.columns.clone());
        let find = |col: &String| mapped.iter().position(|m| m.as_ref() == Some(col)).map(|i| (i, col.clone()));
        let key = std::iter::once(pk.clone()).filter(|k| !k.is_empty()).chain(unique).find_map(|k| k.iter().map(find).collect::<Option<Vec<_>>>());
        let Some(key) = key else {
            self.editing = Editing::ReadOnly(if pk.is_empty() {
                i18n::tf(format!("la tabella \"{}\" non ha una chiave primaria", src.table))
            } else {
                i18n::tf(format!("nei risultati manca la chiave primaria ({})", pk.join(", ")))
            });
            return;
        };
        // the key identifies the row: it stays as it is; binary columns aren't edited as text
        let cols: Vec<Option<String>> = mapped
            .iter()
            .enumerate()
            .map(|(i, m)| m.clone().filter(|_| !key.iter().any(|(k, _)| *k == i) && v.columns.get(i).is_some_and(|c| c.kind != ColumnKind::Binary)))
            .collect();
        for (i, _) in &key {
            if let Some(c) = v.columns.get_mut(*i) {
                c.primary_key = true;
            }
        }
        if cols.iter().all(Option::is_none) {
            self.editing = Editing::ReadOnly(i18n::t("nessuna colonna modificabile").to_string());
            return;
        }
        let table = match &src.schema {
            Some(s) => format!("{s}.{}", info.name),
            None => info.name.clone(),
        };
        self.editing = Editing::Yes(table);
        let editable: Vec<bool> = cols.iter().map(Option::is_some).collect();
        let first = editable.iter().position(|e| *e);
        v.edit = Some(ResultEdit { schema: src.schema, table: info.name, key, cols, editable });
        if self.debug_edit && let Some(c) = first {
            Self::set_result_cell(v, 0, c, Some("Modificato".into()));
        }
        if std::mem::take(&mut self.debug_picker)
            && let Some(c) = v.columns.iter().enumerate().position(|(i, c)| v.edit.as_ref().is_some_and(|e| e.editable[i]) && dbgrid::datetime::DateKind::of(c.kind).is_some())
        {
            v.grid.open_date_picker(0, c as u32);
        }
    }

    /// "Save": shows the statements the pending edits will run.
    fn ask_save(&mut self, env: &Env<'_>) {
        let Some(v) = self.results.iter().find(|v| !v.edits.is_empty()) else { return };
        let (Some(e), Some(cfg)) = (&v.edit, env.cfg(&self.loc.conn)) else { return };
        let d = Dialect::from(cfg.kind);
        let statements: Vec<String> = Self::result_changes(v).iter().map(|c| d.change_sql(e.schema.as_deref(), &e.table, c)).collect();
        if !statements.is_empty() {
            self.confirm = Some(statements);
        }
    }

    /// The UPDATE statements for the pending edits of a result.
    fn result_changes(v: &ResultView) -> Vec<RowChange> {
        let Some(e) = &v.edit else { return Vec::new() };
        let mut rows: Vec<usize> = v.edits.cells.keys().map(|k| k.0).collect();
        rows.sort_unstable();
        rows.dedup();
        rows.into_iter()
            .filter_map(|r| {
                let row = v.rs.rows.get(r)?;
                let key = e.key.iter().map(|(i, name)| Some((name.clone(), row.get(*i)?.clone()))).collect::<Option<Vec<_>>>()?;
                let mut set: Vec<(usize, Value)> = v.edits.cells.iter().filter(|(k, _)| k.0 == r).map(|(k, val)| (k.1, val.clone())).collect();
                set.sort_by_key(|x| x.0);
                let set: Vec<(String, Value)> = set.into_iter().filter_map(|(c, val)| Some((e.cols.get(c)?.clone()?, val))).collect();
                (!set.is_empty()).then_some(RowChange::Update { key, set })
            })
            .collect()
    }

    /// A cell of the result was edited in the grid.
    fn set_result_cell(v: &mut ResultView, row: usize, col: usize, value: Option<String>) {
        if !v.edit.as_ref().is_some_and(|e| e.editable.get(col).copied().unwrap_or(false)) {
            return;
        }
        let val = value.map_or(Value::Null, Value::Text);
        let original = v.rs.rows.get(row).and_then(|r| r.get(col));
        let same = original.is_some_and(|o| *o == val || (!o.is_null() && !val.is_null() && o.to_string() == val.to_string()));
        if same {
            v.edits.cells.remove(&(row, col));
        } else {
            v.edits.cells.insert((row, col), val);
        }
    }

    /// Saves the confirmed edits of the result, in one transaction.
    fn save_result(&mut self, env: &mut Env<'_>) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        let Some(v) = self.results.iter().find(|v| !v.edits.is_empty()) else { return };
        let Some(e) = &v.edit else { return };
        let changes = Self::result_changes(v);
        if changes.is_empty() {
            return;
        }
        self.saving = true;
        let (tab, schema, table) = (self.id, e.schema.clone(), e.table.clone());
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.apply_changes(schema.as_deref(), &table, &changes).await,
                Err(e) => Err(e),
            };
            Msg::Applied { tab, res }
        });
    }

    fn on_applied(&mut self, res: db_core::DbResult<u64>, env: &mut Env<'_>) {
        if !std::mem::take(&mut self.saving) {
            return;
        }
        match res {
            Ok(n) => {
                // the saved values become the result's own
                for v in &mut self.results {
                    for ((r, c), val) in std::mem::take(&mut v.edits.cells) {
                        if let Some(cell) = v.rs.rows.get_mut(r).and_then(|row| row.get_mut(c)) {
                            *cell = val;
                        }
                    }
                }
                self.messages.push((format!("{n} record salvati"), false));
                env.actions.push(Action::Notice(format!("{n} record salvati"), false));
            }
            Err(e) => {
                self.messages.push((format!("Modifiche non salvate: {e}"), true));
                env.actions.push(Action::Notice(format!("Modifiche non salvate: {e}"), true));
            }
        }
    }

    /// The statements of "Save", shown before they run.
    fn confirm_ui(&mut self, ctx: &egui::Context, env: &mut Env<'_>) {
        let Some(statements) = &self.confirm else { return };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique(("result_save", self.id))).show(ctx, |ui| {
            ui.set_max_width(720.0);
            ui.heading(i18n::t("Salvare le modifiche?"));
            ui.add_space(4.0);
            ui.label(i18n::tf(format!("Istruzioni da eseguire in una transazione ({}). Se una di esse interessa più di una riga, non viene salvato niente.", statements.len())));
            ui.add_space(6.0);
            egui::ScrollArea::both().max_height(320.0).auto_shrink([false, true]).show(ui, |ui| {
                for s in statements {
                    ui.add(egui::Label::new(RichText::new(format!("{s};")).monospace()).wrap_mode(egui::TextWrapMode::Extend));
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add(crate::theme::primary(i18n::t("Salva"))).clicked() {
                    answer = Some(true);
                }
                if ui.button(i18n::t("Annulla")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => {
                self.confirm = None;
                self.save_result(env);
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }

    pub fn status_text(&self) -> String {
        match self.running {
            Some(t) => format!("In esecuzione… {:.1}s", t.elapsed().as_secs_f64()),
            None => self.last.clone(),
        }
    }

    /// Code completion state, for the status bar.
    pub fn completion_text(&self) -> String {
        match &self.meta {
            Meta::Idle | Meta::Loading => "Caricamento completamento codice…".into(),
            Meta::Ready(t) => format!("Completamento codice pronto (aggiornato alle {t})"),
            Meta::Failed(e) => format!("Completamento codice non disponibile: {e}"),
        }
    }

    /// "Ln 1, Col 3" at the cursor.
    pub fn cursor_text(&self) -> String {
        let before: String = self.sql.chars().take(self.cursor).collect();
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        format!("Ln {line}, Col {col}")
    }

    pub fn open_builder(&mut self, env: &mut Env<'_>) {
        self.show_builder = true;
        if self.builder.is_some() {
            return;
        }
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        self.builder = Some(QueryBuilder::new(Dialect::from(cfg.kind), self.loc.schema.clone()));
        let (tab, schema) = (self.id, self.loc.schema.clone());
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.list_objects(schema.as_deref()).await,
                Err(e) => Err(e),
            };
            Msg::BuilderTables { tab, res }
        });
    }

    /// Scripted builder state for screenshots (`--builder tables;fields`).
    pub fn debug_builder(&mut self, env: &mut Env<'_>, spec: &str) {
        self.open_builder(env);
        let (tables, fields) = spec.split_once(';').unwrap_or((spec, ""));
        if let Some(b) = self.builder.as_mut() {
            for t in tables.split(',').filter(|t| !t.is_empty()) {
                b.add_table(t, None);
            }
            for f in fields.split(',').filter(|f| !f.is_empty()) {
                b.debug_field(f);
            }
        }
        self.builder_requests(env);
    }

    /// Scripted editor state for screenshots (`--editor "text|"`): `|` marks the cursor,
    /// a trailing `!` opens completion.
    pub fn debug_editor(&mut self, spec: &str) {
        let (spec, complete) = match spec.strip_suffix('!') {
            Some(s) => (s, true),
            None => (spec, false),
        };
        let spec = spec.replace("\\n", "\n");
        let cursor = spec.find('|').map(|b| char_of(&spec, b));
        self.sql = spec.replacen('|', "", 1);
        let c = cursor.unwrap_or_else(|| self.sql.chars().count());
        self.cursor = c;
        self.move_cursor = Some(c);
        self.complete_when_ready = complete;
    }

    /// Sends the table definitions the builder asked for.
    fn builder_requests(&mut self, env: &mut Env<'_>) {
        let Some(b) = self.builder.as_mut() else { return };
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else { return };
        for BuilderRequest::Describe(name) in std::mem::take(&mut b.requests) {
            let (tab, schema) = (self.id, self.loc.schema.clone());
            env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
                let res = match d {
                    Ok(d) => d.describe_table(schema.as_deref(), &name).await,
                    Err(e) => Err(e),
                };
                Msg::BuilderTable { tab, name, res }
            });
        }
    }

    /// Replaces chars `a..b` with `text` and puts the cursor after it.
    fn replace(&mut self, a: usize, b: usize, text: &str) {
        let (ba, bb) = (byte_of(&self.sql, a), byte_of(&self.sql, b));
        self.sql.replace_range(ba..bb, text);
        let c = a + text.chars().count();
        self.cursor = c;
        self.selection = None;
        self.move_cursor = Some(c);
    }

    fn accept(&mut self, env: &Env<'_>, k: usize) {
        let Some(p) = self.popup.take() else { return };
        let Some(it) = p.items.get(k) else { return };
        let text = complete::insert_text(it, self.dialect(env));
        let end = self.cursor.max(p.start);
        self.replace(p.start, end, &text);
    }

    /// Opens, refreshes or closes the completion popup for the text at the cursor.
    fn update_popup(&mut self, env: &mut Env<'_>, forced: bool, typed: bool) {
        let ctx = complete::context(&self.sql, self.cursor);
        let opening = self.popup.is_none();
        if opening {
            let before = self.sql.chars().nth(self.cursor.wrapping_sub(1));
            let trigger = typed && before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
            if !(forced || trigger) {
                return;
            }
        } else if self.popup.as_ref().is_some_and(|p| p.start != ctx.start) && !typed {
            self.popup = None;
            return;
        }
        let forced = forced || self.popup.as_ref().is_some_and(|p| p.forced && p.start == ctx.start);
        let d = self.dialect(env);
        let stmt = statement_at(&self.sql, d, byte_of(&self.sql, self.cursor)).map(|r| self.sql[r].to_string()).unwrap_or_default();
        let mut need = Vec::new();
        let items = self.catalog.candidates(&ctx, &stmt, forced, &mut need);
        let loading = !need.is_empty() && items.is_empty() && ctx.qualifier.is_some();
        if !need.is_empty() {
            self.request_columns(env, need);
        }
        if items.is_empty() && !loading {
            self.popup = None;
            return;
        }
        let (sel, rect) = match &self.popup {
            Some(p) if p.start == ctx.start => (p.items.get(p.sel).and_then(|cur| items.iter().position(|i| i == cur)).unwrap_or(0), p.rect),
            _ => (0, egui::Rect::NOTHING),
        };
        self.popup = Some(Popup { items, sel, start: ctx.start, forced, scroll: true, loading, rect });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        self.confirm_ui(&ui.ctx().clone(), env);
        if self.show_builder {
            let done = self.builder.as_mut().and_then(|b| b.ui(ui));
            self.builder_requests(env);
            match done {
                Some(true) => {
                    if let Some(b) = &self.builder {
                        self.sql = b.sql();
                    }
                    self.show_builder = false;
                }
                Some(false) => self.show_builder = false,
                None => {}
            }
            return;
        }
        if matches!(self.meta, Meta::Idle) {
            self.request_meta(env);
        }
        let d = self.dialect(env);
        let editor_id = egui::Id::unique(("sql-editor", self.id));
        let mut run: Option<Run> = None;
        if std::mem::take(&mut self.pending_run) {
            run = Some(Run::All);
        }
        let mut beautify = false;
        let mut insert: Option<String> = None;
        let mut stop = false;

        // Row 1: file, builder, beautify, snippets.
        ui.horizontal_wrapped(|ui| {
            if icons::button(ui, G::Save, "Salva").on_hover_text(i18n::t("⌘S")).clicked() || ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
                self.save(env, false);
            }
            ui.menu_button("▾", |ui| {
                if icons::menu_item(ui, G::Save, "Salva con nome…").clicked() {
                    self.save(env, true);
                    ui.close();
                }
                if icons::menu_item(ui, G::OpenFolder, "Apri file SQL…").clicked() {
                    self.open_file(env);
                    ui.close();
                }
            });
            ui.separator();
            if icons::button(ui, Db::QueryBuilder, "Query Builder").on_hover_text(i18n::t("Costruisci la query visualmente")).clicked() {
                self.open_builder(env);
            }
            if icons::button(ui, Db::Beautify, "Abbellisci SQL").on_hover_text(i18n::t("Formatta la selezione o tutto lo script (⌘⇧F)")).clicked() {
                beautify = true;
            }
            let snip = icons::button(ui, Db::Snippet, "Snippet ▾").on_hover_text(i18n::t("Inserisci un'istruzione pronta"));
            egui::Popup::menu(&snip).show(|ui| {
                for (name, text) in snippets(d) {
                    if ui.button(name).on_hover_text(RichText::new(&text).monospace()).clicked() {
                        insert = Some(text);
                        ui.close();
                    }
                }
            });
        });
        // Row 2: where to run, run, stop, explain.
        ui.horizontal_wrapped(|ui| {
            self.location_pickers(ui, env);
            ui.separator();
            let idle = self.running.is_none();
            if icons::button_ex(ui, Db::Run, "Esegui", false, idle).on_hover_text(i18n::t("Esegui tutto (⌘↩)")).clicked() {
                run = Some(Run::All);
            }
            ui.add_enabled_ui(idle, |ui| {
                ui.menu_button("▾", |ui| {
                    if icons::menu_item(ui, Db::Run, "Esegui tutto   ⌘↩").clicked() {
                        run = Some(Run::All);
                        ui.close();
                    }
                    if icons::menu_item(ui, Db::RunSelection, "Esegui selezione").clicked() {
                        run = Some(Run::Selection);
                        ui.close();
                    }
                    if icons::menu_item(ui, Db::RunSelection, "Esegui istruzione corrente   ⌘⇧↩").clicked() {
                        run = Some(Run::Current);
                        ui.close();
                    }
                });
            });
            if icons::button_ex(ui, G::StopSquare, "Stop", false, !idle).on_hover_text(i18n::t("Interrompi la query in esecuzione")).clicked() {
                stop = true;
            }
            if icons::button_ex(ui, Db::Explain, "Explain", false, idle).on_hover_text(i18n::t("Piano di esecuzione della selezione o dell'istruzione al cursore")).clicked() {
                run = Some(Run::Explain);
            }
            if !idle {
                ui.spinner();
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            }
        });

        // Keys for the editor, before it sees them.
        let focused = ui.ctx().memory(|m| m.has_focus(editor_id));
        let mut accept: Option<usize> = None;
        let mut force = false;
        if focused {
            let n = self.popup.as_ref().map_or(0, |p| p.items.len());
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter) {
                    run = Some(Run::Current);
                }
                if i.consume_key(Modifiers::COMMAND, Key::Enter) {
                    run = Some(Run::All);
                }
                if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::F) {
                    beautify = true;
                }
                if i.consume_key(Modifiers::CTRL, Key::Space) {
                    force = true;
                }
                if let Some(p) = &mut self.popup {
                    let mut moved = |delta: isize| {
                        if n > 0 {
                            p.sel = (p.sel as isize + delta).clamp(0, n as isize - 1) as usize;
                            p.scroll = true;
                        }
                    };
                    if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                        moved(1);
                    }
                    if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                        moved(-1);
                    }
                    if i.consume_key(Modifiers::NONE, Key::PageDown) {
                        moved(8);
                    }
                    if i.consume_key(Modifiers::NONE, Key::PageUp) {
                        moved(-8);
                    }
                    if n > 0 && (i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab)) {
                        accept = Some(p.sel);
                    }
                    if i.consume_key(Modifiers::NONE, Key::Escape) {
                        self.popup = None;
                    }
                }
            });
        } else if let Some(p) = &self.popup {
            // Focus went elsewhere (but not to the popup itself).
            let over = ui.ctx().pointer_latest_pos().is_some_and(|pos| p.rect.contains(pos));
            if !over && !ui.input(|i| i.pointer.any_down()) {
                self.popup = None;
            }
        }
        if let Some(k) = accept {
            self.accept(env, k);
        }
        if beautify {
            match self.selection.filter(|(a, b)| a < b) {
                Some((a, b)) => {
                    let part: String = self.sql.chars().skip(a).take(b - a).collect();
                    let f = db_core::format::format_sql(&part, d);
                    self.replace(a, b, &f);
                }
                None => {
                    self.sql = db_core::format::format_sql(&self.sql, d);
                    self.cursor = self.cursor.min(self.sql.chars().count());
                    self.move_cursor = Some(self.cursor);
                }
            }
            self.popup = None;
        }
        if let Some(text) = insert {
            let (a, b) = self.selection.unwrap_or((self.cursor, self.cursor));
            self.replace(a, b, &text);
            ui.ctx().memory_mut(|m| m.request_focus(editor_id));
        }
        if let Some(c) = self.move_cursor.take() {
            let mut st = egui::TextEdit::load_state(ui.ctx(), editor_id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(c))));
            st.store(ui.ctx(), editor_id);
            ui.ctx().memory_mut(|m| m.request_focus(editor_id));
        }

        let mut editor_out = None;
        egui::Panel::top(egui::Id::unique(("qedit", self.id))).resizable(true).default_size(260.0).min_size(80.0).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(("qscroll", self.id)).show(ui, |ui| {
                let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
                let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap: f32| {
                    let mut job = egui_extras::syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, buf.as_str(), "sql");
                    job.wrap.max_width = wrap;
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let out = egui::TextEdit::multiline(&mut self.sql)
                    .id(editor_id)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(12)
                    .hint_text(i18n::t("Scrivi qui le istruzioni SQL…  (Ctrl+Spazio: completamento)"))
                    .layouter(&mut layouter)
                    .show(ui);
                editor_out = Some((out.response.changed(), out.cursor_range, out.galley.clone(), out.galley_pos));
            });
        });
        if let Some((changed, range, galley, galley_pos)) = editor_out {
            if let Some(r) = range {
                let (c, o): (usize, usize) = (r.primary.index.into(), r.secondary.index.into());
                self.cursor = c;
                let (a, b) = (c.min(o), c.max(o));
                self.selection = (a != b).then_some((a, b));
            }
            // Scripted (`--editor`): complete once the catalog is loaded, as if just typed.
            let scripted = self.complete_when_ready && matches!(self.meta, Meta::Ready(_)) && self.cursor > 0;
            if scripted {
                self.complete_when_ready = false;
            }
            if changed || force || scripted || self.popup.is_some() {
                self.update_popup(env, force, changed || scripted);
            }
            if let Some(k) = self.popup_ui(ui, &galley, galley_pos) {
                self.accept(env, k);
                ui.ctx().memory_mut(|m| m.request_focus(editor_id));
            }
        }

        if stop {
            self.stop(env);
        }
        if let Some(what) = run {
            self.popup = None;
            self.run(env, what);
        }

        // Result tabs.
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_result, 0, i18n::t("Messaggio"));
            for (i, v) in self.results.iter().enumerate() {
                ui.selectable_value(&mut self.active_result, i + 1, v.label.as_str());
            }
            if let Some(v) = self.active_result.checked_sub(1).and_then(|i| self.results.get(i)) {
                let mut save = false;
                let mut discard = false;
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icons::button(ui, Db::Export, "Esporta risultato").clicked() {
                        env.actions.push(Action::Export(ExportSource::Rows(Box::new(v.rs.clone()), "risultato".into(), self.loc.conn.clone())));
                    }
                    ui.label(RichText::new(i18n::tf(format!("{} righe", v.rs.rows.len()))).small());
                    if !v.edits.is_empty() {
                        ui.separator();
                        if ui.add_enabled(!self.saving, egui::Button::new(i18n::t("Annulla modifiche"))).clicked() {
                            discard = true;
                        }
                        if ui.add_enabled(!self.saving, crate::theme::primary(i18n::t("Salva"))).clicked() {
                            save = true;
                        }
                        ui.label(RichText::new(i18n::tf(format!("{} modifiche in sospeso", v.edits.count()))).small().strong());
                    }
                    if self.active_result == 1 {
                        ui.separator();
                        match &self.editing {
                            Editing::Yes(t) => {
                                ui.label(RichText::new(i18n::tf(format!("Modificabile: tabella {t}"))).small())
                                    .on_hover_text(i18n::t("Doppio clic su una cella per modificarla. Le modifiche si salvano con «Salva», una riga alla volta tramite la sua chiave."));
                            }
                            Editing::ReadOnly(why) => {
                                ui.label(RichText::new(i18n::tf(format!("Sola lettura: {why}"))).small().weak());
                            }
                            Editing::Checking | Editing::No => {}
                        }
                    }
                });
                if save || (self.debug_edit && !v.edits.is_empty()) {
                    self.debug_edit = false;
                    self.ask_save(env);
                }
                if discard && let Some(v) = self.active_result.checked_sub(1).and_then(|i| self.results.get_mut(i)) {
                    v.edits = Edits::default();
                }
            }
        });
        ui.separator();
        match self.active_result.checked_sub(1).and_then(|i| self.results.get_mut(i)) {
            Some(v) => {
                let events = {
                    let edit = v.edit.as_ref().filter(|_| !self.saving).map(|e| (&v.edits, e.editable.as_slice()));
                    let model = ResultModel { rs: &v.rs, columns: &v.columns, edit };
                    v.grid.show(ui, &model)
                };
                for ev in events {
                    if let dbgrid::GridEvent::CellEdited { row, col, value } = ev {
                        Self::set_result_cell(v, row, col, value);
                    }
                }
            }
            None => {
                egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    for (m, err) in &self.messages {
                        let t = RichText::new(i18n::tr(m)).monospace();
                        ui.label(if *err { t.color(egui::Color32::from_rgb(0xC4, 0x2B, 0x1C)) } else { t });
                    }
                });
            }
        }
    }

    /// The completion list under the word being typed; returns the clicked item.
    fn popup_ui(&mut self, ui: &egui::Ui, galley: &egui::Galley, galley_pos: egui::Pos2) -> Option<usize> {
        let p = self.popup.as_mut()?;
        let anchor = galley_pos + galley.pos_from_cursor(CCursor::new(p.start)).left_bottom().to_vec2() + egui::vec2(-22.0, 2.0);
        let mut clicked = None;
        let area = egui::Area::new(egui::Id::unique(("sql-complete", self.id))).order(egui::Order::Foreground).fixed_pos(anchor).constrain(true).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).inner_margin(2.0).show(ui, |ui| {
                let w = 360.0;
                ui.set_width(w);
                if p.loading && p.items.is_empty() {
                    ui.label(RichText::new(i18n::t("Caricamento colonne…")).weak());
                    return;
                }
                let h = (p.items.len() as f32 * 20.0).min(230.0);
                egui::ScrollArea::vertical().min_scrolled_height(h).max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    let small = egui::FontId::proportional(11.0);
                    for (k, it) in p.items.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w - 14.0, 20.0), egui::Sense::click());
                        let sel = k == p.sel;
                        if sel || resp.hovered() {
                            let fill = if sel { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.hovered.weak_bg_fill };
                            ui.painter().rect_filled(rect, 2.0, fill);
                        }
                        let color = if sel { ui.visuals().selection.stroke.color } else { ui.visuals().text_color() };
                        let ir = egui::Rect::from_center_size(egui::pos2(rect.left() + 11.0, rect.center().y), egui::Vec2::splat(14.0));
                        icons::paint(ui.painter(), ir, kind_icon(it.kind), color);
                        ui.painter().text(egui::pos2(rect.left() + 24.0, rect.center().y), egui::Align2::LEFT_CENTER, &it.label, font.clone(), color);
                        if !it.detail.is_empty() {
                            ui.painter().text(egui::pos2(rect.right() - 6.0, rect.center().y), egui::Align2::RIGHT_CENTER, &it.detail, small.clone(), color.gamma_multiply(0.6));
                        }
                        if sel && p.scroll {
                            ui.scroll_to_rect(rect, None);
                        }
                        if resp.clicked() {
                            clicked = Some(k);
                        }
                    }
                });
            });
        });
        p.scroll = false;
        p.rect = area.response.rect;
        clicked
    }

    /// Connection, database and (PostgreSQL / SQL Server) schema the query runs in.
    fn location_pickers(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        let mut new_loc: Option<Location> = None;
        let conn_name = env.cfg(&self.loc.conn).map(|c| c.name.clone()).unwrap_or_default();
        icons::show(ui, Db::Connection, 16.0);
        egui::ComboBox::from_id_salt(("q-conn", self.id)).selected_text(conn_name).width(150.0).show_ui(ui, |ui| {
            for c in env.configs {
                if ui.selectable_label(c.id == self.loc.conn, c.name.as_str()).clicked() && c.id != self.loc.conn {
                    new_loc = Some(Location { conn: c.id.clone(), db: if c.kind == DbKind::Sqlite { String::new() } else { c.database.clone() }, schema: None });
                }
            }
        });
        let kind = env.cfg(&self.loc.conn).map(|c| c.kind);
        if kind != Some(DbKind::Sqlite) {
            icons::show(ui, Db::Database, 16.0);
            let shown = if self.loc.db.is_empty() { "(predefinito)".to_string() } else { self.loc.db.clone() };
            egui::ComboBox::from_id_salt(("q-db", self.id)).selected_text(shown).width(140.0).show_ui(ui, |ui| {
                for db in &self.dbs {
                    if ui.selectable_label(*db == self.loc.db, db.as_str()).clicked() && *db != self.loc.db {
                        new_loc = Some(Location { conn: self.loc.conn.clone(), db: db.clone(), schema: None });
                    }
                }
            });
        }
        if matches!(kind, Some(DbKind::Postgres | DbKind::SqlServer)) && !self.schemas.is_empty() {
            icons::show(ui, Db::Schema, 16.0);
            let default = if kind == Some(DbKind::Postgres) { "public" } else { "dbo" };
            let shown = self.loc.schema.clone().unwrap_or_else(|| default.to_string());
            egui::ComboBox::from_id_salt(("q-schema", self.id)).selected_text(shown.as_str()).width(110.0).show_ui(ui, |ui| {
                for s in &self.schemas {
                    if ui.selectable_label(*s == shown, s.as_str()).clicked() && *s != shown {
                        new_loc = Some(Location { schema: Some(s.clone()), ..self.loc.clone() });
                    }
                }
            });
        }
        if let Some(l) = new_loc {
            if l.conn != self.loc.conn || l.db != self.loc.db {
                self.dbs.clear();
                self.schemas.clear();
            }
            self.loc = l;
            self.reset_meta();
            let _ = env;
        }
    }

    fn open_file(&mut self, env: &mut Env<'_>) {
        if let Some(p) = rfd::FileDialog::new().add_filter("SQL", &["sql"]).pick_file() {
            match std::fs::read_to_string(&p) {
                Ok(t) => {
                    self.sql = t;
                    if let Some(name) = p.file_name() {
                        env.actions.push(Action::SetTitle(self.id, name.to_string_lossy().into_owned()));
                    }
                    self.path = Some(p);
                }
                Err(e) => env.actions.push(Action::Notice(e.to_string(), true)),
            }
        }
    }

    fn save(&mut self, env: &mut Env<'_>, ask: bool) {
        // as in Navicat, the queries are saved with the connection/database: they show up
        // under "Query" in the toolbar (another folder can be chosen in the dialog)
        let dir = crate::store::place_dir("queries", &self.loc.conn, &self.loc.db, self.loc.schema.as_deref());
        let _ = std::fs::create_dir_all(&dir);
        let path = self.path.clone().filter(|_| !ask).or_else(|| rfd::FileDialog::new().add_filter("SQL", &["sql"]).set_directory(&dir).set_file_name("query.sql").save_file());
        if let Some(p) = path {
            match std::fs::write(&p, &self.sql) {
                Ok(()) => {
                    if let Some(name) = p.file_name() {
                        env.actions.push(Action::SetTitle(self.id, name.to_string_lossy().into_owned()));
                    }
                    self.path = Some(p);
                    env.actions.push(Action::Notice("Query salvata".into(), false));
                }
                Err(e) => env.actions.push(Action::Notice(e.to_string(), true)),
            }
        }
    }
}
