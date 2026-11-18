//! The main window: toolbar, connection tree, tab strip, status bar and dialogs.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use db_core::{ConnectionConfig, DbKind, Dialect, ObjectKind, SchemaObject};
use egui::{Color32, RichText};

use crate::backend::{Backend, Msg, TabId};
use crate::dialogs::{ConfirmDialog, ConnDialog, ExportDialog, ExportSource, ImportDialog};
use crate::icons::{self, Db, G, Ico};
use crate::store;
use crate::tabs::automation::AutomationTab;
use crate::tabs::backup::BackupTab;
use crate::tabs::catalog::{CatalogKind, CatalogTab};
use crate::tabs::model::ModelTab;
use crate::tabs::{designer::DesignerTab, objects, query::QueryTab, table::TableTab};
use crate::i18n;

/// A database object address.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ObjRef {
    pub conn: String,
    pub db: String,
    pub schema: Option<String>,
    pub name: String,
    pub kind: ObjectKind,
}

/// A place in the tree that holds objects: connection + database + optional schema.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Location {
    pub conn: String,
    pub db: String,
    pub schema: Option<String>,
}

pub enum Load<T> {
    NotLoaded,
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(t) => Some(t),
            _ => None,
        }
    }
}

pub struct SchemaNode {
    pub name: String,
    pub open: bool,
    pub objects: Load<Vec<SchemaObject>>,
}

pub struct DbNode {
    pub name: String,
    pub open: bool,
    /// `Ready(empty)` for engines without schemas; objects then live in `objects`.
    pub schemas: Load<Vec<SchemaNode>>,
    pub objects: Load<Vec<SchemaObject>>,
}

#[derive(Default)]
pub struct ConnNode {
    pub open: bool,
    pub dbs: Option<Load<Vec<DbNode>>>,
}

/// Side effects requested by tabs and dialogs, applied after the frame's UI.
pub enum Action {
    OpenTable(ObjRef),
    DesignTable(ObjRef),
    NewTable(Location),
    NewQuery(Location),
    /// New query tab with the visual builder open.
    NewBuilder(Location),
    Export(ExportSource),
    Import(ObjRef),
    Refresh(Location),
    Drop(ObjRef),
    Truncate(ObjRef),
    Notice(String, bool),
    SetTitle(TabId, String),
    /// Adds the object to the favorites or removes it.
    ToggleFavorite(ObjRef),
    /// Opens a saved query in a query tab.
    OpenQueryFile(Location, std::path::PathBuf),
    /// Deletes a saved query (after asking).
    DeleteQueryFile(std::path::PathBuf),
    /// Runs a scheduled job now.
    RunJob(u64),
}

/// Is `obj` among the favorites?
pub fn is_favorite(favs: &[store::Favorite], obj: &ObjRef) -> bool {
    favs.iter().any(|f| f.conn == obj.conn && f.db == obj.db && f.schema == obj.schema && f.name == obj.name && f.kind == obj.kind)
}

/// What tabs may touch while drawing.
pub struct Env<'a> {
    pub backend: &'a Backend,
    pub configs: &'a [ConnectionConfig],
    pub actions: &'a mut Vec<Action>,
}

impl Env<'_> {
    pub fn cfg(&self, id: &str) -> Option<&ConnectionConfig> {
        self.configs.iter().find(|c| c.id == id)
    }
    pub fn dialect(&self, id: &str) -> Dialect {
        self.cfg(id).map_or(Dialect::Sqlite, |c| Dialect::from(c.kind))
    }
}

pub enum Tab {
    Objects,
    Table(Box<TableTab>),
    Query(Box<QueryTab>),
    Designer(Box<DesignerTab>),
    Catalog(Box<CatalogTab>),
    Backup(Box<BackupTab>),
    Automation(Box<AutomationTab>),
    Model(Box<ModelTab>),
    /// «Impostazioni AI»: the settings page (a WebView), as in OpenSlate.
    AiSettings,
}

pub struct TabEntry {
    pub id: TabId,
    pub title: String,
    pub tab: Tab,
}

pub struct App {
    backend: Backend,
    configs: Vec<ConnectionConfig>,
    tree: HashMap<String, ConnNode>,
    /// Location shown by the Objects tab, and the selected object.
    pub loc: Option<Location>,
    selected: Option<ObjRef>,
    tabs: Vec<TabEntry>,
    active: usize,
    next_id: TabId,
    conn_dialog: Option<ConnDialog>,
    export_dialog: Option<ExportDialog>,
    import_dialog: Option<ImportDialog>,
    confirm: Option<ConfirmDialog>,
    notices: Vec<(String, bool, Instant)>,
    objects_filter: String,
    /// Object kinds the Objects tab lists (toolbar Tabella / Vista / Funzione / Trigger).
    objects_kinds: Vec<ObjectKind>,
    actions: Vec<Action>,
    cli: crate::cli::Cli,
    updater: crate::update_ui::Updater,
    /// The Objects tab shows the saved queries instead of the objects (toolbar "Query").
    objects_queries: bool,
    query_sel: Option<std::path::PathBuf>,
    favorites: Vec<store::Favorite>,
    /// Tree: search text and "only favorites".
    tree_filter: String,
    fav_only: bool,
    /// Scheduled jobs (Automazione) and the ones running.
    jobs: Vec<store::Job>,
    running_jobs: HashSet<u64>,
    show_about: bool,
    tree_gen: u64,
    /// The AI chat and settings pages (WebViews inside the window).
    embeds: crate::embed::Embeds,
    /// The AI chat is open on the right.
    chat_open: bool,
    chat_rect: Option<egui::Rect>,
    settings_rect: Option<egui::Rect>,
    /// macOS: the native menu bar (kept alive).
    #[cfg(target_os = "macos")]
    _mac_menu: Option<muda::Menu>,
}

pub fn icon(kind: ObjectKind) -> Db {
    match kind {
        ObjectKind::Table => Db::Table,
        ObjectKind::View => Db::View,
        ObjectKind::Function => Db::Function,
        ObjectKind::Procedure => Db::Procedure,
        ObjectKind::Trigger => Db::Trigger,
    }
}

fn kind_color(kind: DbKind) -> Color32 {
    match kind {
        DbKind::MySql => Color32::from_rgb(0x00, 0x75, 0x8F),
        DbKind::MariaDb => Color32::from_rgb(0xA0, 0x6A, 0x3B),
        DbKind::Postgres => Color32::from_rgb(0x33, 0x67, 0x91),
        DbKind::SqlServer => Color32::from_rgb(0xC4, 0x2B, 0x1C),
        DbKind::Sqlite => Color32::from_rgb(0x0F, 0x80, 0xCC),
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let backend = Backend::new(cc.egui_ctx.clone()).map_err(|e| e.to_string())?;
        let mut configs = store::load();
        // `dbcraft file.db` or `dbcraft postgres://user:pw@host/db` opens a temporary (unsaved)
        // connection.
        let cli_opts = crate::cli::Cli::parse();
        if cli_opts.dark {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
        }
        let mut startup_error = None;
        let cli = cli_opts.db.clone().and_then(|arg| {
            let parsed = if arg.contains("://") {
                ConnectionConfig::from_url(&arg)
            } else {
                let name = std::path::Path::new(&arg).file_name().map_or_else(|| arg.clone(), |n| n.to_string_lossy().into_owned());
                Ok(ConnectionConfig { name, kind: DbKind::Sqlite, database: arg.clone(), ..Default::default() })
            };
            match parsed {
                Ok(c) => Some(ConnectionConfig { id: format!("{}cli", store::TRANSIENT), ..c }),
                Err(e) => {
                    startup_error = Some(e);
                    None
                }
            }
        });
        let cli_id = cli.as_ref().map(|c| c.id.clone());
        configs.extend(cli);
        let mut app = App {
            backend,
            tree: configs.iter().map(|c| (c.id.clone(), ConnNode::default())).collect(),
            configs,
            loc: None,
            selected: None,
            tabs: vec![TabEntry { id: 0, title: "Oggetti".into(), tab: Tab::Objects }],
            active: 0,
            next_id: 1,
            conn_dialog: None,
            export_dialog: None,
            import_dialog: None,
            confirm: None,
            notices: Vec::new(),
            objects_filter: String::new(),
            objects_kinds: vec![ObjectKind::Table, ObjectKind::View],
            actions: Vec::new(),
            cli: cli_opts,
            updater: crate::update_ui::Updater::new(),
            objects_queries: false,
            query_sel: None,
            favorites: store::load_favorites(),
            tree_filter: String::new(),
            fav_only: false,
            jobs: store::load_jobs(),
            running_jobs: HashSet::new(),
            show_about: false,
            tree_gen: 0,
            embeds: crate::embed::Embeds::new(),
            chat_open: false,
            chat_rect: None,
            settings_rect: None,
            #[cfg(target_os = "macos")]
            _mac_menu: mac_menu(),
        };
        // the AI chat: its server and its database tools on DbCraft's runtime
        crate::dbagent::set_runtime(app.backend.handle());
        crate::web::start(&app.backend.handle(), &cc.egui_ctx);
        if let Some(id) = cli_id {
            app.connect(&id);
        }
        if let Some(e) = startup_error {
            app.notify(e, true);
        }
        if let Some(kind) = app.cli.new_connection.take() {
            let kind = match kind.to_ascii_lowercase().as_str() {
                "postgres" | "postgresql" => Some(DbKind::Postgres),
                "mysql" => Some(DbKind::MySql),
                "mariadb" => Some(DbKind::MariaDb),
                "sqlserver" | "mssql" => Some(DbKind::SqlServer),
                "sqlite" => Some(DbKind::Sqlite),
                _ => None,
            };
            if let Some(k) = kind {
                let d = ConnDialog::new(k);
                app.conn_dialog = Some(if app.cli.ssh_tab {
                    d.on_ssh_tab()
                } else if app.cli.http_tab {
                    d.on_http_tab()
                } else if app.cli.ssl_tab {
                    d.on_ssl_tab()
                } else {
                    d
                });
            }
        }
        Ok(app)
    }

    fn cfg(&self, id: &str) -> Option<&ConnectionConfig> {
        self.configs.iter().find(|c| c.id == id)
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notices.push((i18n::tf(text.into()), error, Instant::now()));
    }

    fn add_tab(&mut self, title: String, tab: Tab) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(TabEntry { id, title, tab });
        self.active = self.tabs.len() - 1;
        id
    }

    // ----- connection tree loading -----

    fn connect(&mut self, conn: &str) {
        let Some(cfg) = self.cfg(conn).cloned() else { return };
        let node = self.tree.entry(conn.to_string()).or_default();
        node.open = true;
        node.dbs = Some(Load::Loading);
        let id = cfg.id.clone();
        self.backend.with_driver(&cfg, "", move |d| async move {
            let res = match d {
                Ok(d) => d.list_databases().await,
                Err(e) => Err(e),
            };
            Msg::Connected { conn: id, res }
        });
    }

    fn disconnect(&mut self, conn: &str) {
        self.backend.disconnect(conn);
        if let Some(n) = self.tree.get_mut(conn) {
            n.dbs = None;
            n.open = false;
        }
    }

    fn load_db(&mut self, conn: &str, db: &str) {
        let Some(cfg) = self.cfg(conn).cloned() else { return };
        if let Some(d) = self.db_node(conn, db) {
            d.schemas = Load::Loading;
            d.open = true;
        }
        let (c, n) = (conn.to_string(), db.to_string());
        self.backend.with_driver(&cfg, db, move |d| async move {
            let res = match d {
                Ok(d) => d.list_schemas().await,
                Err(e) => Err(e),
            };
            Msg::Schemas { conn: c, db: n, res }
        });
    }

    fn load_objects(&mut self, loc: &Location) {
        let Some(cfg) = self.cfg(&loc.conn).cloned() else { return };
        if let Some(slot) = self.objects_slot(loc) {
            *slot = Load::Loading;
        }
        let l = loc.clone();
        self.backend.with_driver(&cfg, &loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.list_objects(l.schema.as_deref()).await,
                Err(e) => Err(e),
            };
            Msg::Objects { conn: l.conn, db: l.db, schema: l.schema, res }
        });
    }

    fn db_node(&mut self, conn: &str, db: &str) -> Option<&mut DbNode> {
        match self.tree.get_mut(conn)?.dbs.as_mut()? {
            Load::Ready(dbs) => dbs.iter_mut().find(|d| d.name == db),
            _ => None,
        }
    }

    fn objects_slot(&mut self, loc: &Location) -> Option<&mut Load<Vec<SchemaObject>>> {
        let db = self.db_node(&loc.conn, &loc.db)?;
        match &loc.schema {
            None => Some(&mut db.objects),
            Some(s) => match &mut db.schemas {
                Load::Ready(v) => v.iter_mut().find(|x| &x.name == s).map(|x| &mut x.objects),
                _ => None,
            },
        }
    }

    pub fn objects_at(&self, loc: &Location) -> Option<&Load<Vec<SchemaObject>>> {
        let dbs = self.tree.get(&loc.conn)?.dbs.as_ref()?.ready()?;
        let db = dbs.iter().find(|d| d.name == loc.db)?;
        match &loc.schema {
            None => Some(&db.objects),
            Some(s) => db.schemas.ready()?.iter().find(|x| &x.name == s).map(|x| &x.objects),
        }
    }

    fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Connected { conn, res } => {
                let kind = self.cfg(&conn).map(|c| c.kind);
                let default_db = self.cfg(&conn).map(|c| c.database.clone()).unwrap_or_default();
                let Some(node) = self.tree.get_mut(&conn) else { return };
                match res {
                    Ok(dbs) => {
                        node.dbs = Some(Load::Ready(
                            dbs.into_iter().map(|name| DbNode { name, open: false, schemas: Load::NotLoaded, objects: Load::NotLoaded }).collect(),
                        ));
                        // Open the configured database straight away (SQLite: its only one).
                        let auto = match kind {
                            Some(DbKind::Sqlite) => Some("main".to_string()),
                            _ if !default_db.is_empty() => Some(default_db),
                            _ => None,
                        };
                        if let Some(db) = auto
                            && self.db_node(&conn, &db).is_some()
                        {
                            self.load_db(&conn, &db);
                        }
                    }
                    Err(e) => {
                        node.dbs = None;
                        node.open = false;
                        let name = self.cfg(&conn).map(|c| c.name.clone()).unwrap_or_default();
                        self.notify(format!("{name}: {e}"), true);
                    }
                }
            }
            Msg::Schemas { conn, db, res } => match res {
                Ok(schemas) if schemas.is_empty() => {
                    if let Some(d) = self.db_node(&conn, &db) {
                        d.schemas = Load::Ready(Vec::new());
                    }
                    let loc = Location { conn, db, schema: None };
                    self.load_objects(&loc);
                    self.loc.get_or_insert(loc);
                }
                Ok(schemas) => {
                    let default = schemas.iter().find(|s| ["public", "dbo"].contains(&s.as_str())).or(schemas.first()).cloned();
                    if let Some(d) = self.db_node(&conn, &db) {
                        d.schemas = Load::Ready(schemas.into_iter().map(|name| SchemaNode { name, open: false, objects: Load::NotLoaded }).collect());
                    }
                    if let Some(s) = default {
                        if let Some(Load::Ready(v)) = self.db_node(&conn, &db).map(|d| &mut d.schemas)
                            && let Some(n) = v.iter_mut().find(|x| x.name == s)
                        {
                            n.open = true;
                        }
                        let loc = Location { conn, db, schema: Some(s) };
                        self.load_objects(&loc);
                        self.loc.get_or_insert(loc);
                    }
                }
                Err(e) => {
                    if let Some(d) = self.db_node(&conn, &db) {
                        d.schemas = Load::Failed(e.to_string());
                    }
                    self.notify(e.to_string(), true);
                }
            },
            Msg::Objects { conn, db, schema, res } => {
                let loc = Location { conn, db, schema };
                if let Some(slot) = self.objects_slot(&loc) {
                    *slot = match res {
                        Ok(v) => Load::Ready(v),
                        Err(e) => Load::Failed(e.to_string()),
                    };
                }
            }
            Msg::ObjectsChanged { conn, db, schema } => self.load_objects(&Location { conn, db, schema }),
            Msg::Notice { text, error } => self.notify(text, error),
            Msg::ImportInfo { res } => {
                if let Some(d) = self.import_dialog.as_mut() {
                    d.on_info(res);
                }
            }
            Msg::Imported { res } => {
                if let Some(d) = self.import_dialog.as_mut() {
                    d.running = false;
                }
                match res {
                    Ok(n) => {
                        self.notify(format!("Importati {n} record"), false);
                        self.import_dialog = None;
                    }
                    Err(e) => self.notify(format!("Importazione fallita: {e}"), true),
                }
            }
            Msg::Dumped { job: Some(job), res, .. } => self.job_finished(job, res),
            Msg::JobDone { job, res } => self.job_finished(job, res),
            Msg::DumpProgress { tab: None, .. } => {}
            other => {
                let tab = match &other {
                    Msg::TableData { tab, .. } | Msg::Applied { tab, .. } | Msg::QueryDone { tab, .. } | Msg::QueryMeta { tab, .. } | Msg::QueryColumns { tab, .. } | Msg::BuilderTables { tab, .. } | Msg::BuilderTable { tab, .. } | Msg::DesignLoaded { tab, .. } | Msg::DesignSaved { tab, .. } | Msg::Catalog { tab, .. } | Msg::Model { tab, .. } => *tab,
                    Msg::DumpProgress { tab: Some(tab), .. } | Msg::Dumped { tab: Some(tab), .. } => *tab,
                    _ => return,
                };
                let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut self.actions };
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    match &mut t.tab {
                        Tab::Table(t) => t.handle(other, &mut env),
                        Tab::Query(t) => t.handle(other, &mut env),
                        Tab::Designer(t) => t.handle(other, &mut env),
                        Tab::Catalog(t) => t.handle(other, &mut env),
                        Tab::Backup(t) => t.handle(other, &mut env),
                        Tab::Model(t) => t.handle(other),
                        Tab::Objects | Tab::Automation(_) | Tab::AiSettings => {}
                    }
                }
            }
        }
    }

    fn apply_actions(&mut self) {
        for a in std::mem::take(&mut self.actions) {
            match a {
                Action::OpenTable(obj) => {
                    if let Some(i) = self.tabs.iter().position(|t| matches!(&t.tab, Tab::Table(x) if x.obj == obj)) {
                        self.active = i;
                        continue;
                    }
                    let Some(cfg) = self.cfg(&obj.conn).cloned() else { continue };
                    let title = if obj.db.is_empty() || obj.db == "main" { format!("{} @{}", obj.name, cfg.name) } else { format!("{} @{} ({})", obj.name, obj.db, cfg.name) };
                    let id = self.next_id;
                    let mut tab = TableTab::new(id, obj);
                    if !self.cli.table_ui.is_empty() {
                        tab.debug_setup(&std::mem::take(&mut self.cli.table_ui));
                    }
                    let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut Vec::new() };
                    tab.reload(&mut env, true);
                    self.add_tab(title, Tab::Table(Box::new(tab)));
                }
                Action::DesignTable(obj) => {
                    let id = self.next_id;
                    let title = format!("{} (Progetta)", obj.name);
                    let dialect = self.cfg(&obj.conn).map_or(Dialect::Sqlite, |c| Dialect::from(c.kind));
                    let mut tab = DesignerTab::edit(id, obj, dialect);
                    let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut Vec::new() };
                    tab.load(&mut env);
                    self.add_tab(title, Tab::Designer(Box::new(tab)));
                }
                Action::NewTable(loc) => {
                    let id = self.next_id;
                    let dialect = self.cfg(&loc.conn).map_or(Dialect::Sqlite, |c| Dialect::from(c.kind));
                    self.add_tab("Senza titolo (Tabella)".into(), Tab::Designer(Box::new(DesignerTab::new_table(id, loc, dialect))));
                }
                Action::NewQuery(loc) => {
                    let id = self.next_id;
                    let n = self.tabs.iter().filter(|t| matches!(t.tab, Tab::Query(_))).count() + 1;
                    let title = format!("Query {n} @{}", if loc.db.is_empty() { self.cfg(&loc.conn).map(|c| c.name.clone()).unwrap_or_default() } else { loc.db.clone() });
                    self.add_tab(title, Tab::Query(Box::new(QueryTab::new(id, loc))));
                }
                Action::NewBuilder(loc) => {
                    let id = self.next_id;
                    let n = self.tabs.iter().filter(|t| matches!(t.tab, Tab::Query(_))).count() + 1;
                    let mut tab = QueryTab::new(id, loc);
                    let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut Vec::new() };
                    tab.open_builder(&mut env);
                    self.add_tab(format!("Query {n}"), Tab::Query(Box::new(tab)));
                }
                Action::Export(src) => self.export_dialog = Some(ExportDialog::new(src)),
                Action::Import(obj) => {
                    let mut d = ImportDialog::new(obj.clone());
                    if let Some(cfg) = self.cfg(&obj.conn).cloned() {
                        self.backend.with_driver(&cfg, &obj.db, move |drv| async move {
                            let res = match drv {
                                Ok(drv) => drv.describe_table(obj.schema.as_deref(), &obj.name).await,
                                Err(e) => Err(e),
                            };
                            Msg::ImportInfo { res }
                        });
                    } else {
                        d.running = false;
                    }
                    self.import_dialog = Some(d);
                }
                Action::Refresh(loc) => self.load_objects(&loc),
                Action::Drop(obj) => {
                    let view = obj.kind == ObjectKind::View;
                    let what = if view { "la vista" } else { "la tabella" };
                    self.confirm = Some(ConfirmDialog::new(format!("Eliminare {what} \"{}\"? L'operazione non è reversibile.", obj.name), ConfirmKind::Drop(obj)));
                }
                Action::Truncate(obj) => {
                    self.confirm = Some(ConfirmDialog::new(format!("Svuotare la tabella \"{}\"? Tutti i record verranno eliminati.", obj.name), ConfirmKind::Truncate(obj)));
                }
                Action::Notice(t, e) => self.notify(t, e),
                Action::ToggleFavorite(obj) => self.toggle_favorite(&obj),
                Action::OpenQueryFile(loc, path) => {
                    if let Some(i) = self.tabs.iter().position(|t| matches!(&t.tab, Tab::Query(q) if q.path.as_ref() == Some(&path))) {
                        self.active = i;
                        continue;
                    }
                    match std::fs::read_to_string(&path) {
                        Ok(sql) => {
                            let id = self.next_id;
                            let mut tab = QueryTab::new(id, loc);
                            tab.set_sql(sql, false);
                            tab.path = Some(path.clone());
                            let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Query".into());
                            self.add_tab(title, Tab::Query(Box::new(tab)));
                        }
                        Err(e) => self.notify(e.to_string(), true),
                    }
                }
                Action::DeleteQueryFile(path) => {
                    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    self.confirm = Some(ConfirmDialog::new(format!("Eliminare la query \"{name}\"?"), ConfirmKind::DeleteFile(path)));
                }
                Action::RunJob(id) => self.run_job(id),
                Action::SetTitle(id, title) => {
                    if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                        t.title = title;
                    }
                }
            }
        }
    }

    fn run_confirmed(&mut self, kind: ConfirmKind) {
        match kind {
            ConfirmKind::Drop(obj) | ConfirmKind::Truncate(obj) if self.cfg(&obj.conn).is_none() => {}
            ConfirmKind::Drop(obj) => self.run_ddl(obj.clone(), |d| d.drop_table(obj.schema.as_deref(), &obj.name, obj.kind == ObjectKind::View), "Eliminato"),
            ConfirmKind::Truncate(obj) => self.run_ddl(obj.clone(), |d| d.truncate_table(obj.schema.as_deref(), &obj.name), "Svuotata"),
            ConfirmKind::DeleteFile(path) => {
                if let Err(e) = std::fs::remove_file(&path) {
                    self.notify(e.to_string(), true);
                }
            }
            ConfirmKind::DeleteConn(id) => {
                self.backend.disconnect(&id);
                store::delete_secrets(&id);
                self.configs.retain(|c| c.id != id);
                self.tree.remove(&id);
                if self.loc.as_ref().is_some_and(|l| l.conn == id) {
                    self.loc = None;
                }
                if let Err(e) = store::save(&self.configs) {
                    self.notify(e, true);
                }
            }
        }
    }

    fn run_ddl(&mut self, obj: ObjRef, sql: impl FnOnce(Dialect) -> String, done: &'static str) {
        let Some(cfg) = self.cfg(&obj.conn).cloned() else { return };
        let sql = sql(Dialect::from(cfg.kind));
        let db = obj.db.clone();
        self.backend.with_driver(&cfg, &db, move |d| async move {
            match d {
                Ok(d) => match d.execute(&sql).await {
                    Ok(_) => Msg::ObjectsChanged { conn: obj.conn, db: obj.db, schema: obj.schema },
                    Err(e) => Msg::Notice { text: e.to_string(), error: true },
                },
                Err(e) => Msg::Notice { text: format!("{done}: {e}"), error: true },
            }
        });
    }

    // ----- UI -----

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let conn_btn = icons::tile(ui, Db::Connection, "Connessione");
            egui::Popup::menu(&conn_btn).show(|ui| self.new_connection_menu(ui));
            let loc = self.loc.clone();
            if icons::tile(ui, Db::Query, "Nuova query").clicked() {
                match loc.clone() {
                    Some(l) => self.actions.push(Action::NewQuery(l)),
                    None => self.notify("Apri prima una connessione/database", true),
                }
            }
            ui.separator();
            // As in Navicat: these show that kind of object in the Objects tab.
            for (ic, label, kinds) in [
                (Db::Table, "Tabella", vec![ObjectKind::Table]),
                (Db::View, "Vista", vec![ObjectKind::View]),
                (Db::Function, "Funzione", vec![ObjectKind::Function, ObjectKind::Procedure]),
            ] {
                let selected = self.active == 0 && !self.objects_queries && self.objects_kinds == kinds;
                let r = icons::tile(ui, ic, label);
                if selected {
                    selected_frame(ui, &r);
                }
                if r.clicked() {
                    self.show_kinds(kinds);
                }
            }
            let r = icons::tile(ui, Db::User, "Utente");
            if r.clicked() {
                self.open_catalog(CatalogKind::Users);
            }
            let r = icons::tile(ui, Db::Others, "Altri ▾");
            if self.active == 0 && !self.objects_queries && self.objects_kinds == [ObjectKind::Trigger] {
                selected_frame(ui, &r);
            }
            egui::Popup::menu(&r).show(|ui| self.others_menu(ui));
            let r = icons::tile(ui, Db::Query, "Query");
            if self.active == 0 && self.objects_queries {
                selected_frame(ui, &r);
            }
            if r.clicked() {
                self.objects_queries = true;
                self.active = 0;
            }
            if icons::tile(ui, Db::Backup, "Backup").clicked() {
                self.open_backup();
            }
            if icons::tile(ui, Db::Automation, "Automazione").clicked() {
                self.open_automation();
            }
            if icons::tile(ui, Db::Model, "Modello").clicked() {
                self.open_model();
            }
            let r = icons::tile(ui, Db::Sparkle, "Chat AI").on_hover_text(i18n::t("L'assistente AI: scrive e spiega le query, legge la struttura e i dati dei database"));
            if self.chat_open {
                selected_frame(ui, &r);
            }
            if r.clicked() {
                self.chat_open = !self.chat_open;
                if self.chat_open {
                    self.embeds.focus(crate::embed::Slot::Chat);
                }
            }
            ui.separator();
            if icons::tile(ui, Db::NewTable, "Nuova tabella").clicked() {
                match loc.clone() {
                    Some(l) => self.actions.push(Action::NewTable(l)),
                    None => self.notify("Apri prima un database", true),
                }
            }
            if icons::tile(ui, Db::QueryBuilder, "Query Builder").clicked() {
                match loc.clone() {
                    Some(l) => self.actions.push(Action::NewBuilder(l)),
                    None => self.notify("Apri prima una connessione/database", true),
                }
            }
            ui.separator();
            let sel_table = self.selected.clone().filter(|o| o.kind == ObjectKind::Table);
            if icons::tile(ui, Db::Import, "Importa").clicked() {
                match sel_table.clone() {
                    Some(o) => self.actions.push(Action::Import(o)),
                    None => self.notify("Seleziona una tabella da importare", true),
                }
            }
            if icons::tile(ui, Db::Export, "Esporta").clicked() {
                match self.selected.clone().filter(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View)) {
                    Some(o) => self.actions.push(Action::Export(ExportSource::Table(o))),
                    None => self.notify("Seleziona una tabella o vista da esportare", true),
                }
            }
            ui.separator();
            if icons::tile(ui, Db::Refresh, "Aggiorna").clicked()
                && let Some(l) = loc
            {
                self.actions.push(Action::Refresh(l));
            }
            let upd = icons::tile(ui, Db::Update, "Aggiornamenti");
            egui::Popup::menu(&upd).show(|ui| self.updater.menu(ui));
            let dark = ui.visuals().dark_mode;
            if icons::tile(ui, if dark { Db::Sun } else { Db::Moon }, if dark { "Tema chiaro" } else { "Tema scuro" }).clicked() {
                ui.ctx().set_theme(if dark { egui::Theme::Light } else { egui::Theme::Dark });
            }
        });
    }

    fn new_connection_menu(&mut self, ui: &mut egui::Ui) {
        for k in DbKind::ALL {
            let r = icons::menu_item(ui, Db::Database, &format!("{}…", k.label()));
            ui.painter().circle_filled(r.rect.left_center() + egui::vec2(18.0, 4.0), 3.0, kind_color(k));
            if r.clicked() {
                self.conn_dialog = Some(ConnDialog::new(k));
            }
        }
    }

    /// "Altri ▾": trigger and the other objects this server has.
    fn others_menu(&mut self, ui: &mut egui::Ui) {
        if icons::menu_item(ui, Db::Trigger, "Trigger").clicked() {
            self.show_kinds(vec![ObjectKind::Trigger]);
        }
        let kind = self.loc.as_ref().and_then(|l| self.cfg(&l.conn)).map(|c| c.kind);
        match kind {
            Some(k) => {
                for c in CatalogKind::others(k) {
                    if icons::menu_item(ui, c.icon(), c.label()).clicked() {
                        self.open_catalog(c);
                    }
                }
            }
            None => {
                ui.label(RichText::new(i18n::t("Apri un database per gli altri oggetti")).weak());
            }
        }
    }

    fn show_kinds(&mut self, kinds: Vec<ObjectKind>) {
        self.objects_kinds = kinds;
        self.objects_queries = false;
        self.active = 0;
    }

    /// Activates the tab `pred` finds, or adds the one `make` builds.
    fn open_unique(&mut self, pred: impl Fn(&Tab) -> bool, title: String, make: impl FnOnce(TabId) -> Tab) {
        if let Some(i) = self.tabs.iter().position(|t| pred(&t.tab)) {
            self.active = i;
            return;
        }
        let id = self.next_id;
        let tab = make(id);
        self.add_tab(title, tab);
    }

    fn open_catalog(&mut self, kind: CatalogKind) {
        let Some(loc) = self.loc.clone() else {
            self.notify("Apri prima una connessione/database", true);
            return;
        };
        // users belong to the server: one tab per connection
        let loc = if kind == CatalogKind::Users { Location { conn: loc.conn, db: loc.db, schema: None } } else { loc };
        let conn_name = self.cfg(&loc.conn).map(|c| c.name.clone()).unwrap_or_default();
        let probe = CatalogTab::new(0, loc.clone(), kind);
        let title = probe.title(&conn_name);
        self.open_unique(
            |t| matches!(t, Tab::Catalog(c) if c.kind == kind && c.loc.conn == loc.conn && (kind == CatalogKind::Users || c.loc == loc)),
            title,
            |id| Tab::Catalog(Box::new(CatalogTab::new(id, loc.clone(), kind))),
        );
    }

    fn open_backup(&mut self) {
        let Some(loc) = self.loc.clone() else {
            self.notify("Apri prima una connessione/database", true);
            return;
        };
        let name = if loc.db.is_empty() { self.cfg(&loc.conn).map(|c| c.name.clone()).unwrap_or_default() } else { loc.db.clone() };
        let title = match &loc.schema {
            Some(s) => format!("Backup @{name}.{s}"),
            None => format!("Backup @{name}"),
        };
        self.open_unique(|t| matches!(t, Tab::Backup(b) if b.loc == loc), title, |id| Tab::Backup(Box::new(BackupTab::new(id, loc.clone()))));
    }

    fn open_automation(&mut self) {
        self.open_unique(|t| matches!(t, Tab::Automation(_)), "Automazione".into(), |id| Tab::Automation(Box::new(AutomationTab::new(id))));
    }

    fn open_model(&mut self) {
        let Some(loc) = self.loc.clone() else {
            self.notify("Apri prima una connessione/database", true);
            return;
        };
        let name = if loc.db.is_empty() { self.cfg(&loc.conn).map(|c| c.name.clone()).unwrap_or_default() } else { loc.db.clone() };
        self.open_unique(|t| matches!(t, Tab::Model(m) if m.loc == loc), format!("Modello @{name}"), |id| Tab::Model(Box::new(ModelTab::new(id, loc.clone()))));
    }

    fn toggle_favorite(&mut self, obj: &ObjRef) {
        if is_favorite(&self.favorites, obj) {
            self.favorites.retain(|f| !(f.conn == obj.conn && f.db == obj.db && f.schema == obj.schema && f.name == obj.name && f.kind == obj.kind));
        } else {
            self.favorites.push(store::Favorite { conn: obj.conn.clone(), db: obj.db.clone(), schema: obj.schema.clone(), name: obj.name.clone(), kind: obj.kind });
        }
        if let Err(e) = store::save_favorites(&self.favorites) {
            self.notify(e, true);
        }
    }

    /// Opens a favorite: connects if needed, selects it and opens tables and views.
    fn open_favorite(&mut self, f: &store::Favorite) {
        let obj = ObjRef { conn: f.conn.clone(), db: f.db.clone(), schema: f.schema.clone(), name: f.name.clone(), kind: f.kind };
        if self.cfg(&f.conn).is_none() {
            self.notify("La connessione di questo preferito non esiste più", true);
            return;
        }
        if self.tree.get(&f.conn).is_none_or(|n| n.dbs.is_none()) {
            self.connect(&f.conn);
        }
        self.loc = Some(Location { conn: obj.conn.clone(), db: obj.db.clone(), schema: obj.schema.clone() });
        self.selected = Some(obj.clone());
        if matches!(obj.kind, ObjectKind::Table | ObjectKind::View) {
            self.actions.push(Action::OpenTable(obj));
        } else {
            self.show_kinds(vec![f.kind]);
        }
    }

    // ----- scheduled jobs (Automazione) -----

    fn run_job(&mut self, id: u64) {
        let Some(job) = self.jobs.iter().find(|j| j.id == id).cloned() else { return };
        if self.running_jobs.contains(&id) {
            return;
        }
        let Some(cfg) = self.cfg(&job.conn).cloned() else {
            self.job_finished(id, Err("la connessione non esiste più".into()));
            return;
        };
        self.running_jobs.insert(id);
        let loc = Location { conn: job.conn.clone(), db: job.db.clone(), schema: job.schema.clone() };
        match &job.action {
            store::JobAction::Backup => {
                crate::dump::start_backup(&self.backend, &cfg, &loc, None, Some(id));
            }
            store::JobAction::Query { sql } => {
                let sql = sql.clone();
                self.backend.with_driver(&cfg, &loc.db, move |d| async move {
                    let res = match d {
                        Ok(d) => d.execute(&sql).await.map(|r| {
                            let affected: u64 = r.iter().filter_map(|x| x.affected).sum();
                            let rows: usize = r.iter().map(|x| x.rows.len()).sum();
                            format!("{} istruzioni, {affected} righe modificate, {rows} righe lette", r.len())
                        }),
                        Err(e) => Err(e),
                    };
                    Msg::JobDone { job: id, res: res.map_err(|e| e.to_string()) }
                });
            }
        }
    }

    fn job_finished(&mut self, id: u64, res: Result<String, String>) {
        self.running_jobs.remove(&id);
        let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) else { return };
        job.last_run = Some(store::now());
        let (text, error) = match res {
            Ok(t) => (t, false),
            Err(e) => (format!("Errore: {e}"), true),
        };
        job.last_result = Some(text.clone());
        let name = job.name.clone();
        if let Err(e) = store::save_jobs(&self.jobs) {
            self.notify(e, true);
        }
        self.notify(format!("Attività «{name}»: {text}"), error);
    }

    /// Starts the enabled jobs whose time has come.
    fn run_due_jobs(&mut self, ctx: &egui::Context) {
        if !self.jobs.iter().any(|j| j.enabled) {
            return;
        }
        let now = store::now();
        let due: Vec<u64> = self.jobs.iter().filter(|j| j.enabled && j.next_run <= now && !self.running_jobs.contains(&j.id)).map(|j| j.id).collect();
        if !due.is_empty() {
            for j in self.jobs.iter_mut().filter(|j| due.contains(&j.id)) {
                j.next_run = j.schedule.next_after(now);
            }
            if let Err(e) = store::save_jobs(&self.jobs) {
                self.notify(e, true);
            }
            for id in due {
                self.run_job(id);
            }
        }
        // wake up for the next one even when nothing moves
        let next = self.jobs.iter().filter(|j| j.enabled).map(|j| j.next_run).min().unwrap_or(now + 60);
        ctx.request_repaint_after(Duration::from_secs((next - now).clamp(1, 60) as u64));
    }

    // ----- menu bar -----

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(i18n::t("File"), |ui| {
                ui.menu_button(i18n::t("Nuova connessione"), |ui| self.new_connection_menu(ui));
                if icons::menu_item(ui, Db::Query, "Nuova query").clicked() {
                    match self.loc.clone() {
                        Some(l) => self.actions.push(Action::NewQuery(l)),
                        None => self.notify("Apri prima una connessione/database", true),
                    }
                }
                if icons::menu_item(ui, G::OpenFolder, "Apri file SQL…").clicked()
                    && let Some(l) = self.loc.clone()
                    && let Some(p) = rfd::FileDialog::new().add_filter("SQL", &["sql"]).pick_file()
                {
                    self.actions.push(Action::OpenQueryFile(l, p));
                }
                ui.separator();
                let can_close = self.active > 0;
                if ui.add_enabled(can_close, egui::Button::new(i18n::t("Chiudi scheda"))).clicked() {
                    self.close_tab(self.active);
                }
                ui.separator();
                if ui.button(i18n::t("Esci")).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(i18n::t("Modifica"), |ui| {
                let sel = self.selected.clone();
                if ui.add_enabled(sel.is_some(), egui::Button::new(i18n::t("Copia nome oggetto"))).clicked()
                    && let Some(o) = sel
                {
                    ui.ctx().copy_text(o.name);
                }
                if ui.button(i18n::t("Cerca nell'albero")).clicked() {
                    ui.ctx().memory_mut(|m| m.request_focus(egui::Id::unique("tree_search")));
                }
            });
            ui.menu_button(i18n::t("Visualizza"), |ui| {
                if icons::menu_item(ui, Db::Objects, "Oggetti").clicked() {
                    self.active = 0;
                }
                ui.separator();
                for (ic, label, kinds) in [
                    (Db::Table, "Tabelle", vec![ObjectKind::Table]),
                    (Db::View, "Viste", vec![ObjectKind::View]),
                    (Db::Function, "Funzioni", vec![ObjectKind::Function, ObjectKind::Procedure]),
                    (Db::Trigger, "Trigger", vec![ObjectKind::Trigger]),
                    (Db::Objects, "Tabelle e viste", vec![ObjectKind::Table, ObjectKind::View]),
                ] {
                    if icons::menu_item(ui, ic, label).clicked() {
                        self.show_kinds(kinds);
                    }
                }
                if icons::menu_item(ui, Db::Query, "Query salvate").clicked() {
                    self.objects_queries = true;
                    self.active = 0;
                }
                ui.separator();
                ui.menu_button(i18n::t("Lingua"), |ui| self.language_menu(ui));
                ui.checkbox(&mut self.fav_only, i18n::t("Solo preferiti nell'albero"));
                let dark = ui.visuals().dark_mode;
                if ui.button(if dark { i18n::t("Tema chiaro") } else { i18n::t("Tema scuro") }).clicked() {
                    ui.ctx().set_theme(if dark { egui::Theme::Light } else { egui::Theme::Dark });
                }
            });
            ui.menu_button(i18n::t("Preferiti"), |ui| {
                let sel = self.selected.clone();
                let label = match &sel {
                    Some(o) if is_favorite(&self.favorites, o) => format!("Rimuovi «{}» dai preferiti", o.name),
                    Some(o) => format!("Aggiungi «{}» ai preferiti", o.name),
                    None => "Aggiungi ai preferiti (seleziona un oggetto)".into(),
                };
                if ui.add_enabled(sel.is_some(), egui::Button::new(label)).clicked()
                    && let Some(o) = sel
                {
                    self.toggle_favorite(&o);
                }
                ui.separator();
                if self.favorites.is_empty() {
                    ui.label(RichText::new(i18n::t("Nessun preferito: clic destro su un oggetto › Aggiungi ai preferiti")).weak());
                }
                let mut open = None;
                for f in &self.favorites {
                    let conn = self.cfg(&f.conn).map(|c| c.name.clone()).unwrap_or_else(|| "?".into());
                    let place = if f.db.is_empty() || f.db == "main" { conn } else { format!("{} › {conn}", f.db) };
                    if icons::menu_item(ui, icon(f.kind), &format!("{}   ({place})", f.name)).clicked() {
                        open = Some(f.clone());
                    }
                }
                if let Some(f) = open {
                    self.open_favorite(&f);
                }
                if !self.favorites.is_empty() {
                    ui.separator();
                    if ui.button(i18n::t("Svuota preferiti")).clicked() {
                        self.favorites.clear();
                        if let Err(e) = store::save_favorites(&self.favorites) {
                            self.notify(e, true);
                        }
                    }
                }
            });
            ui.menu_button(i18n::t("Strumenti"), |ui| {
                if icons::menu_item(ui, Db::User, "Utenti").clicked() {
                    self.open_catalog(CatalogKind::Users);
                }
                ui.menu_button(i18n::t("Altri oggetti"), |ui| self.others_menu(ui));
                if icons::menu_item(ui, Db::Backup, "Backup").clicked() {
                    self.open_backup();
                }
                if icons::menu_item(ui, Db::Automation, "Automazione").clicked() {
                    self.open_automation();
                }
                if icons::menu_item(ui, Db::Model, "Modello").clicked() {
                    self.open_model();
                }
                ui.separator();
                if icons::menu_item(ui, Db::Sparkle, "Chat AI").clicked() {
                    self.chat_open = true;
                }
                if icons::menu_item(ui, Db::Sparkle, "Impostazioni AI").clicked() {
                    self.open_ai_settings();
                }
                if icons::menu_item(ui, Db::QueryBuilder, "Query Builder").clicked()
                    && let Some(l) = self.loc.clone()
                {
                    self.actions.push(Action::NewBuilder(l));
                }
                ui.separator();
                ui.menu_button(i18n::t("Aggiornamenti"), |ui| self.updater.menu(ui));
                if icons::menu_item(ui, G::Folder, "Cartella delle impostazioni").clicked()
                    && let Some(d) = store::config_dir()
                {
                    let _ = std::fs::create_dir_all(&d);
                    crate::selfupdate::open_url(&d.to_string_lossy());
                }
            });
            ui.menu_button(i18n::t("Finestra"), |ui| {
                let mut go = None;
                for (i, t) in self.tabs.iter().enumerate() {
                    if ui.selectable_label(i == self.active, t.title.as_str()).clicked() {
                        go = Some(i);
                    }
                }
                if let Some(i) = go {
                    self.active = i;
                }
                ui.separator();
                if ui.add_enabled(self.tabs.len() > 1, egui::Button::new(i18n::t("Chiudi tutte le schede"))).clicked() {
                    for i in (1..self.tabs.len()).rev() {
                        self.close_tab(i);
                    }
                }
            });
            ui.menu_button(i18n::t("Aiuto"), |ui| {
                if ui.button(i18n::t("Sito di DbCraft (GitHub)")).clicked() {
                    crate::selfupdate::open_url(&format!("https://github.com/{}", crate::update::REPO));
                }
                if ui.button(i18n::t("Novità delle versioni")).clicked() {
                    crate::selfupdate::open_url(&format!("https://github.com/{}/releases", crate::update::REPO));
                }
                ui.separator();
                if ui.button(i18n::t("Informazioni su DbCraft")).clicked() {
                    self.show_about = true;
                }
            });
        });
    }

    /// Visualizza › Lingua: the system's or one of the interface languages (as in OpenSlate).
    fn language_menu(&mut self, ui: &mut egui::Ui) {
        use crate::i18n::Lang;
        let current = self.updater.settings.ui_lang.clone();
        let auto = format!("{} ({})", i18n::t("Automatica, come il sistema"), i18n::system().native());
        let mut choice = None;
        if ui.selectable_label(current == "auto" || Lang::from_code(&current).is_none(), auto).clicked() {
            choice = Some("auto".to_string());
        }
        ui.separator();
        for l in Lang::ALL {
            if ui.selectable_label(current == l.code(), l.native()).clicked() {
                choice = Some(l.code().to_string());
            }
        }
        if let Some(code) = choice {
            self.updater.settings.ui_lang = code;
            self.updater.settings.save();
            i18n::set_lang(i18n::resolve(&self.updater.settings.ui_lang));
            crate::fonts::apply(ui.ctx());
            ui.close();
        }
    }

    // ----- AI chat (as in OpenSlate: pages in WebViews) -----

    /// The page URL of the local server, with the theme.
    fn page_url(&self, ctx: &egui::Context, page: &str) -> Option<String> {
        let theme = if ctx.global_style().visuals.dark_mode { "dark" } else { "light" };
        let sep = if page.contains('?') { '&' } else { '?' };
        crate::web::origin().map(|o| format!("{o}{page}{sep}embed=1&theme={theme}"))
    }

    fn open_ai_settings(&mut self) {
        self.open_unique(|t| matches!(t, Tab::AiSettings), "Impostazioni AI".into(), |_| Tab::AiSettings);
    }

    /// Each frame: what the chat needs to know, what it asked to do, and the messages of the pages.
    fn ai_tick(&mut self, ctx: &egui::Context) {
        use crate::embed::Slot;
        #[cfg(target_os = "macos")]
        while let Ok(ev) = muda::MenuEvent::receiver().try_recv() {
            if ev.id() == "quit" {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        self.embeds.set_wake(ctx);
        crate::dbagent::set_connections(&self.configs);
        match &self.loc {
            Some(l) => {
                let cfg = self.cfg(&l.conn);
                let name = cfg.map(|c| c.name.clone()).unwrap_or_default();
                let kind = cfg.map(|c| c.kind.label()).unwrap_or("");
                let mut title = if l.db.is_empty() { name.clone() } else { format!("{} ({name}, {kind})", l.db) };
                if let Some(s) = &l.schema {
                    title = format!("{}.{s} ({name}, {kind})", l.db);
                }
                crate::ai::set_context(&title, &l.conn, &l.db, l.schema.as_deref());
            }
            None => crate::ai::set_context("", "", "", None),
        }
        // queries the AI opened in the editor
        for (conn, db, schema, sql, title) in crate::dbagent::take_to_open() {
            let id = self.next_id;
            let mut tab = QueryTab::new(id, Location { conn, db, schema });
            tab.set_sql(sql, false);
            self.add_tab(title, Tab::Query(Box::new(tab)));
        }
        for (slot, v) in self.embeds.poll() {
            match v["cmd"].as_str().unwrap_or("") {
                "close_pane" => self.chat_open = false,
                "open_settings" => self.open_ai_settings(),
                "close_settings" => {
                    if let Some(i) = self.tabs.iter().position(|t| matches!(t.tab, Tab::AiSettings)) {
                        self.close_tab(i);
                    }
                }
                "config_saved" => self.embeds.eval(Slot::Chat, "window.wsChat && wsChat.reloadConfig()"),
                // «Apri nell'editor» on a SQL block of an answer
                "open_sql" if slot == Slot::Chat => match self.loc.clone() {
                    Some(loc) => {
                        let id = self.next_id;
                        let mut tab = QueryTab::new(id, loc);
                        tab.set_sql(v["sql"].as_str().unwrap_or("").trim().to_string(), false);
                        let n = self.tabs.iter().filter(|t| matches!(t.tab, Tab::Query(_))).count() + 1;
                        self.add_tab(format!("Query {n} (AI)"), Tab::Query(Box::new(tab)));
                    }
                    None => self.notify("Apri prima una connessione/database", true),
                },
                _ => {}
            }
        }
        // a click on DbCraft's own controls: the keyboard comes back to the app
        if ctx.input(|i| i.pointer.any_pressed())
            && let Some(p) = ctx.input(|i| i.pointer.interact_pos())
            && !self.chat_rect.is_some_and(|r| r.contains(p))
            && !self.settings_rect.is_some_and(|r| r.contains(p))
        {
            self.embeds.focus_parent();
        }
        // the pages follow the theme
        let dark = ctx.global_style().visuals.dark_mode;
        let id = egui::Id::unique("ai_pages_dark");
        if ctx.data(|d| d.get_temp::<bool>(id)) != Some(dark) {
            ctx.data_mut(|d| d.insert_temp(id, dark));
            let js = format!("document.documentElement.dataset.theme = {}", if dark { "'dark'" } else { "'light'" });
            self.embeds.eval(Slot::Chat, &js);
            self.embeds.eval(Slot::Settings, &js);
        }
    }

    /// The chat on the right (before the central panel: side panels go first).
    fn chat_pane(&mut self, root: &mut egui::Ui, frame: &eframe::Frame) {
        use crate::embed::Slot;
        if !self.chat_open || crate::web::origin().is_none() {
            self.chat_rect = None;
            return;
        }
        if self.embeds.failed.is_some() {
            // no WebView here: the chat in the browser
            if let Some(url) = self.page_url(root.ctx(), "chat") {
                crate::selfupdate::open_url(&url.replace("embed=1", "embed=0"));
            }
            self.chat_open = false;
            return;
        }
        let bg = root.visuals().panel_fill;
        if !self.embeds.has(Slot::Chat)
            && let Some(url) = self.page_url(root.ctx(), "chat?pane=1")
            && let Some(origin) = crate::web::origin()
        {
            self.embeds.ensure(Slot::Chat, frame, &origin, &url, bg);
        }
        let total = root.available_rect_before_wrap().width();
        let max = (total - 360.0).max(340.0);
        let r = egui::Panel::right("ai_chat")
            .resizable(true)
            .default_size((total * 0.32).clamp(340.0, 520.0))
            .size_range(320.0..=max)
            .frame(egui::Frame::NONE.fill(bg).stroke(egui::Stroke::new(1.0, root.visuals().widgets.noninteractive.bg_stroke.color)))
            .show(root, |ui| {
                let r = ui.max_rect();
                ui.allocate_rect(r, egui::Sense::hover());
                if !self.embeds.has(Slot::Chat) {
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, i18n::t("Apro la chat…"), egui::FontId::proportional(12.0), ui.visuals().weak_text_color());
                }
                r
            });
        // a free strip on the side of the border that widens the panel
        self.chat_rect = Some(r.inner.with_min_x(r.inner.min.x + 4.0));
    }

    /// Puts the pages in place; hides them under menus, popups and dialogs (a WebView is always
    /// above what egui draws).
    fn place_pages(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        use crate::embed::Slot;
        // the settings tab: its page
        let settings_active = matches!(self.tabs.get(self.active).map(|t| &t.tab), Some(Tab::AiSettings));
        if settings_active
            && !self.embeds.has(Slot::Settings)
            && self.embeds.failed.is_none()
            && let (Some(url), Some(origin)) = (self.page_url(ctx, "impostazioni"), crate::web::origin())
        {
            let bg = ctx.global_style().visuals.panel_fill;
            self.embeds.ensure(Slot::Settings, frame, &origin, &url, bg);
        }
        let covers: Vec<egui::Rect> = ctx.memory(|m| {
            m.areas()
                .visible_layer_ids()
                .into_iter()
                .filter(|l| matches!(l.order, egui::Order::Foreground | egui::Order::Middle))
                .filter_map(|l| m.area_rect(l.id))
                .collect()
        });
        let modal = self.conn_dialog.is_some() || self.export_dialog.is_some() || self.import_dialog.is_some() || self.confirm.is_some() || self.show_about;
        let free = |r: Option<egui::Rect>| r.filter(|r| !modal && !covers.iter().any(|c| c.intersects(*r)));
        self.embeds.set_origin(ctx.input(|i| i.viewport().inner_rect.map(|r| r.min)));
        let chat = free(self.chat_rect);
        let settings = free(self.settings_rect.take().filter(|_| settings_active));
        self.embeds.place(Slot::Chat, chat);
        self.embeds.place(Slot::Settings, settings);
        if settings.is_none() && !settings_active && self.embeds.has(Slot::Settings) {
            self.embeds.place(Slot::Settings, None);
        }
    }

    fn close_tab(&mut self, i: usize) {
        if i == 0 || i >= self.tabs.len() {
            return;
        }
        let dirty = match &self.tabs[i].tab {
            Tab::Table(t) => !t.edits.is_empty(),
            _ => false,
        };
        if dirty {
            self.notify("La tabella ha modifiche non salvate: applicale o annullale prima di chiudere", true);
            return;
        }
        self.tabs.remove(i);
        if self.active >= i {
            self.active = self.active.saturating_sub(1);
        }
    }

    fn tree_ui(&mut self, ui: &mut egui::Ui) {
        let mut connect = None;
        let mut disconnect = None;
        let mut edit = None;
        let mut delete = None;
        let mut load_db = None;
        let mut load_objects: Option<Location> = None;
        let mut select_loc: Option<Location> = None;
        let mut select_obj: Option<ObjRef> = None;
        let mut open_obj: Option<ObjRef> = None;
        let mut collapse = false;

        // bottom of the tree, as in Navicat: search, only favorites, collapse all
        egui::Panel::bottom("tree_footer").exact_size(30.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                icons::show(ui, G::Search, 14.0);
                let w = (ui.available_width() - 56.0).max(60.0);
                let r = ui.add(egui::TextEdit::singleline(&mut self.tree_filter).id(egui::Id::unique("tree_search")).hint_text(i18n::t("Cerca")).desired_width(w));
                if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.tree_filter.clear();
                }
                let star = icons::button_ex(ui, Db::Star, "", self.fav_only, true).on_hover_text(if self.fav_only { i18n::t("Mostra tutti gli oggetti") } else { i18n::t("Mostra solo i preferiti") });
                if star.clicked() {
                    self.fav_only = !self.fav_only;
                }
                if icons::small(ui, G::Collapse, "Comprimi tutto", true).clicked() {
                    collapse = true;
                }
            });
        });
        let filter = TreeFilter { text: self.tree_filter.trim().to_lowercase(), fav_only: self.fav_only, favs: &self.favorites, generation: self.tree_gen };

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            if self.configs.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(i18n::t("Nessuna connessione.\nUsa «Connessione» nella barra in alto.")).weak());
            }
            for cfg in &self.configs {
                let node = self.tree.entry(cfg.id.clone()).or_default();
                let connected = matches!(node.dbs, Some(Load::Ready(_)));
                let loading = matches!(node.dbs, Some(Load::Loading));
                let r = tree_row(ui, 0, Some(node.open), Db::Connection.into(), Some(if connected { kind_color(cfg.kind) } else { Color32::GRAY }), &cfg.name, false);
                if r.toggled {
                    if node.dbs.is_none() {
                        connect = Some(cfg.id.clone());
                    } else {
                        node.open = !node.open;
                    }
                }
                if r.resp.double_clicked() && node.dbs.is_none() {
                    connect = Some(cfg.id.clone());
                }
                r.resp.context_menu(|ui| {
                    if connected || loading {
                        if ui.button(i18n::t("Chiudi connessione")).clicked() {
                            disconnect = Some(cfg.id.clone());
                        }
                    } else if ui.button(i18n::t("Apri connessione")).clicked() {
                        connect = Some(cfg.id.clone());
                    }
                    if ui.button(i18n::t("Modifica connessione…")).clicked() {
                        edit = Some(cfg.id.clone());
                    }
                    if ui.button(i18n::t("Elimina connessione")).clicked() {
                        delete = Some(cfg.id.clone());
                    }
                    if connected && ui.button(i18n::t("Nuova query")).clicked() {
                        self.actions.push(Action::NewQuery(Location { conn: cfg.id.clone(), db: String::new(), schema: None }));
                    }
                });
                if loading {
                    ui.horizontal(|ui| {
                        ui.add_space(30.0);
                        ui.spinner();
                    });
                }
                if !node.open {
                    continue;
                }
                let Some(Load::Ready(dbs)) = &mut node.dbs else { continue };
                for db in dbs.iter_mut() {
                    let active = db.open && !matches!(db.schemas, Load::NotLoaded);
                    let r = tree_row(ui, 1, Some(db.open), if active { Db::DatabaseOpen.into() } else { Db::Database.into() }, None, &db.name, false);
                    let here = Location { conn: cfg.id.clone(), db: db.name.clone(), schema: None };
                    if r.toggled || r.resp.double_clicked() {
                        if matches!(db.schemas, Load::NotLoaded | Load::Failed(_)) {
                            load_db = Some((cfg.id.clone(), db.name.clone()));
                        } else {
                            db.open = !db.open;
                        }
                    }
                    if r.resp.clicked() && db.schemas.ready().is_some_and(Vec::is_empty) {
                        select_loc = Some(here.clone());
                    }
                    r.resp.context_menu(|ui| {
                        if ui.button(i18n::t("Nuova query")).clicked() {
                            self.actions.push(Action::NewQuery(here.clone()));
                        }
                        if db.schemas.ready().is_some_and(Vec::is_empty) {
                            if ui.button(i18n::t("Nuova tabella")).clicked() {
                                self.actions.push(Action::NewTable(here.clone()));
                            }
                            if ui.button(i18n::t("Aggiorna")).clicked() {
                                load_objects = Some(here.clone());
                            }
                        }
                    });
                    if !db.open {
                        continue;
                    }
                    match &mut db.schemas {
                        Load::Loading => {
                            ui.horizontal(|ui| {
                                ui.add_space(46.0);
                                ui.spinner();
                            });
                        }
                        Load::Failed(e) => {
                            ui.horizontal(|ui| {
                                ui.add_space(46.0);
                                ui.colored_label(Color32::RED, e.as_str());
                            });
                        }
                        Load::Ready(schemas) if schemas.is_empty() => {
                            objects_tree(ui, 2, &db.objects, &here, self.selected.as_ref(), &mut select_obj, &mut open_obj, &mut self.actions, &filter);
                        }
                        Load::Ready(schemas) => {
                            for s in schemas.iter_mut() {
                                let sl = Location { conn: cfg.id.clone(), db: db.name.clone(), schema: Some(s.name.clone()) };
                                let r = tree_row(ui, 2, Some(s.open), Db::Schema.into(), None, &s.name, self.loc.as_ref() == Some(&sl));
                                if r.toggled || r.resp.double_clicked() {
                                    s.open = !s.open;
                                    if s.open && matches!(s.objects, Load::NotLoaded | Load::Failed(_)) {
                                        load_objects = Some(sl.clone());
                                    }
                                }
                                if r.resp.clicked() {
                                    select_loc = Some(sl.clone());
                                    if matches!(s.objects, Load::NotLoaded) {
                                        load_objects = Some(sl.clone());
                                    }
                                }
                                r.resp.context_menu(|ui| {
                                    if ui.button(i18n::t("Nuova query")).clicked() {
                                        self.actions.push(Action::NewQuery(sl.clone()));
                                    }
                                    if ui.button(i18n::t("Nuova tabella")).clicked() {
                                        self.actions.push(Action::NewTable(sl.clone()));
                                    }
                                    if ui.button(i18n::t("Aggiorna")).clicked() {
                                        load_objects = Some(sl.clone());
                                    }
                                });
                                if s.open {
                                    objects_tree(ui, 3, &s.objects, &sl, self.selected.as_ref(), &mut select_obj, &mut open_obj, &mut self.actions, &filter);
                                }
                            }
                        }
                        Load::NotLoaded => {}
                    }
                }
            }
        });

        if collapse {
            for n in self.tree.values_mut() {
                if let Some(Load::Ready(dbs)) = &mut n.dbs {
                    for d in dbs.iter_mut() {
                        d.open = false;
                        if let Load::Ready(ss) = &mut d.schemas {
                            for s in ss.iter_mut() {
                                s.open = false;
                            }
                        }
                    }
                }
            }
            self.tree_gen += 1;
        }
        if let Some(c) = connect {
            self.connect(&c);
        }
        if let Some(c) = disconnect {
            self.disconnect(&c);
        }
        if let Some((c, d)) = load_db {
            self.load_db(&c, &d);
        }
        if let Some(l) = load_objects {
            self.load_objects(&l);
        }
        if let Some(l) = select_loc {
            self.loc = Some(l);
            self.active = 0;
        }
        if let Some(o) = select_obj {
            self.loc = Some(Location { conn: o.conn.clone(), db: o.db.clone(), schema: o.schema.clone() });
            self.selected = Some(o);
        }
        if let Some(o) = open_obj {
            match o.kind {
                ObjectKind::Table | ObjectKind::View => self.actions.push(Action::OpenTable(o)),
                _ => {}
            }
        }
        if let Some(id) = edit
            && let Some(cfg) = self.cfg(&id).cloned()
        {
            self.conn_dialog = Some(ConnDialog::edit(cfg));
        }
        if let Some(id) = delete {
            let name = self.cfg(&id).map(|c| c.name.clone()).unwrap_or_default();
            self.confirm = Some(ConfirmDialog::new(format!("Eliminare la connessione \"{name}\"?"), ConfirmKind::DeleteConn(id)));
        }
    }

    fn tab_strip(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        egui::ScrollArea::horizontal().id_salt("tabstrip").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (i, t) in self.tabs.iter().enumerate() {
                    let selected = i == self.active;
                    let glyph: Ico = match &t.tab {
                        Tab::Objects => Db::Objects.into(),
                        Tab::Table(_) => Db::Table.into(),
                        Tab::Query(_) => Db::Query.into(),
                        Tab::Designer(_) => Db::Design.into(),
                        Tab::Catalog(c) => c.kind.icon(),
                        Tab::Backup(_) => Db::Backup.into(),
                        Tab::Automation(_) => Db::Automation.into(),
                        Tab::Model(_) => Db::Model.into(),
                        Tab::AiSettings => Db::Sparkle.into(),
                    };
                    let frame = egui::Frame::new()
                        .fill(if selected { ui.visuals().extreme_bg_color } else { ui.visuals().faint_bg_color })
                        .inner_margin(egui::Margin::symmetric(10, 5))
                        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            icons::show(ui, glyph, 15.0);
                            let label = ui.add(egui::Label::new(RichText::new(i18n::tr(&t.title)).strong_if(selected)).sense(egui::Sense::click()));
                            if label.clicked() {
                                self.active = i;
                            }
                            if label.middle_clicked() && i > 0 {
                                close = Some(i);
                            }
                            if i > 0 && close_button(ui).clicked() {
                                close = Some(i);
                            }
                        });
                    });
                }
            });
        });
        if let Some(i) = close {
            self.close_tab(i);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(l) = &self.loc {
                let name = self.cfg(&l.conn).map(|c| c.name.clone()).unwrap_or_default();
                icons::show(ui, Db::Connection, 13.0);
                ui.label(RichText::new(name).small());
                if !l.db.is_empty() {
                    ui.label(RichText::new("›").small().weak());
                    icons::show(ui, Db::Database, 13.0);
                    ui.label(RichText::new(l.db.as_str()).small());
                }
                if let Some(sc) = &l.schema {
                    ui.label(RichText::new("›").small().weak());
                    icons::show(ui, Db::Schema, 13.0);
                    ui.label(RichText::new(sc.as_str()).small());
                }
            }
            if self.active == 0
                && let Some(l) = &self.loc
            {
                let text = if self.objects_queries {
                    let n = store::list_files(&store::place_dir("queries", &l.conn, &l.db, l.schema.as_deref()), "sql").len();
                    format!("{n} query")
                } else if let Some(Load::Ready(objs)) = self.objects_at(l) {
                    let n = objs.iter().filter(|o| self.objects_kinds.contains(&o.kind)).count();
                    let what = match self.objects_kinds.as_slice() {
                        [ObjectKind::Table] => "tabelle",
                        [ObjectKind::View] => "viste",
                        [ObjectKind::Trigger] => "trigger",
                        [ObjectKind::Function, ..] => "funzioni e procedure",
                        _ => "oggetti",
                    };
                    let what = i18n::t(what);
                    format!("{n} {what}")
                } else {
                    String::new()
                };
                if !text.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(i18n::tf(text)).small());
                }
            }
            if let Some(TabEntry { tab: Tab::Table(t), .. }) = self.tabs.get(self.active) {
                ui.separator();
                ui.label(RichText::new(i18n::tf(t.status_text())).small());
            }
            if let Some(TabEntry { tab: Tab::Query(t), .. }) = self.tabs.get(self.active) {
                ui.separator();
                ui.label(RichText::new(i18n::tf(t.status_text())).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(t.cursor_text()).small());
                    ui.separator();
                    ui.label(RichText::new(i18n::tf(t.completion_text())).small().weak());
                });
            }
        });
    }

    fn notices_ui(&mut self, ctx: &egui::Context) {
        self.notices.retain(|n| n.2.elapsed() < Duration::from_secs(if n.1 { 10 } else { 4 }));
        if self.notices.is_empty() {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        egui::Area::new(egui::Id::unique("notices")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -34.0)).show(ctx, |ui| {
            let mut dismiss = None;
            for (i, (text, error, _)) in self.notices.iter().enumerate() {
                egui::Frame::popup(ui.style()).fill(if *error { Color32::from_rgb(0xFB, 0xE3, 0xE1) } else { Color32::from_rgb(0xE3, 0xF6, 0xE8) }).show(ui, |ui| {
                    ui.set_max_width(420.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                        icons::paint(ui.painter(), r, if *error { G::Info } else { G::Check }, Color32::BLACK);
                        ui.add(egui::Label::new(RichText::new(text).color(Color32::BLACK)).wrap());
                        if close_button(ui).clicked() {
                            dismiss = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = dismiss {
                self.notices.remove(i);
            }
        });
    }

    /// Scripted steps from the command line (`--open-table`, `--query`).
    fn automation(&mut self) {
        let Some(loc) = self.loc.clone() else { return };
        if !loc.conn.starts_with(store::TRANSIENT) {
            return;
        }
        if let Some(t) = self.cli.open_table.clone()
            && let Some(Load::Ready(objs)) = self.objects_at(&loc)
        {
            let found = objs.iter().find(|o| o.name == t).map(|o| ObjRef { conn: loc.conn.clone(), db: loc.db.clone(), schema: loc.schema.clone(), name: o.name.clone(), kind: o.kind });
            self.cli.open_table = None;
            if let Some(obj) = found {
                self.selected = Some(obj.clone());
                self.actions.push(Action::OpenTable(obj));
            }
        }
        if let Some(spec) = self.cli.builder.take() {
            let id = self.next_id;
            let mut tab = QueryTab::new(id, loc.clone());
            let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut self.actions };
            tab.debug_builder(&mut env, &spec);
            self.add_tab("Query 1".into(), Tab::Query(Box::new(tab)));
        }
        if let Some(spec) = self.cli.editor.take() {
            let id = self.next_id;
            let mut tab = QueryTab::new(id, loc.clone());
            tab.debug_editor(&spec);
            self.add_tab("Query 1".into(), Tab::Query(Box::new(tab)));
        }
        if let Some(f) = self.cli.tree_filter.take() {
            self.tree_filter = f;
        }
        if self.objects_at(&loc).is_some_and(|o| matches!(o, Load::Ready(_)))
            && let Some(what) = self.cli.open.take()
        {
            match what.as_str() {
                "users" => self.open_catalog(CatalogKind::Users),
                "backup" => self.open_backup(),
                "automation" => self.open_automation(),
                "model" => self.open_model(),
                "queries" => {
                    self.objects_queries = true;
                    self.active = 0;
                }
                "about" => self.show_about = true,
                "chat" => self.chat_open = true,
                "ai-settings" => self.open_ai_settings(),
                other => {
                    if let Some(k) = other.strip_prefix("others:") {
                        let kind = [CatalogKind::Indexes, CatalogKind::Sequences, CatalogKind::Events, CatalogKind::Types, CatalogKind::Extensions, CatalogKind::Synonyms]
                            .into_iter()
                            .find(|c| c.label().eq_ignore_ascii_case(k));
                        if let Some(c) = kind {
                            self.open_catalog(c);
                        }
                    }
                }
            }
        }
        if let Some(sql) = self.cli.query.take() {
            let id = self.next_id;
            let mut tab = QueryTab::new(id, loc);
            tab.set_sql(sql, true);
            self.add_tab("Query 1".into(), Tab::Query(Box::new(tab)));
        }
    }

    fn about(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut close = false;
        egui::Modal::new(egui::Id::unique("about")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.vertical_centered(|ui| {
                icons::show(ui, Db::Database, 40.0);
                ui.heading(i18n::tf(format!("DbCraft {}", crate::update::current())));
                ui.label(i18n::t("Client per MySQL, MariaDB, PostgreSQL, SQL Server e SQLite"));
                ui.label(RichText::new(i18n::t("Open source · MIT OR Apache-2.0")).weak());
                ui.hyperlink_to(i18n::t("github.com/rmpaolillo/DbCraft"), format!("https://github.com/{}", crate::update::REPO));
                ui.add_space(6.0);
                if ui.button(i18n::t("Chiudi")).clicked() {
                    close = true;
                }
            });
        });
        if close {
            self.show_about = false;
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(d) = self.conn_dialog.as_mut() {
            match d.show(ctx, &self.backend) {
                Some(crate::dialogs::ConnOutcome::Save(cfg)) => {
                    let cfg = *cfg;
                    if let Err(e) = store::save_secrets(&cfg) {
                        self.notices.push((format!("Portachiavi: {e}"), true, Instant::now()));
                    }
                    let id = cfg.id.clone();
                    match self.configs.iter_mut().find(|c| c.id == id) {
                        Some(c) => {
                            *c = cfg;
                            self.backend.disconnect(&id);
                            if let Some(n) = self.tree.get_mut(&id) {
                                n.dbs = None;
                                n.open = false;
                            }
                        }
                        None => {
                            self.configs.push(cfg);
                            self.tree.insert(id, ConnNode::default());
                        }
                    }
                    if let Err(e) = store::save(&self.configs) {
                        self.notify(e, true);
                    }
                    self.conn_dialog = None;
                }
                Some(crate::dialogs::ConnOutcome::Cancel) => self.conn_dialog = None,
                None => {}
            }
        }
        if let Some(d) = self.export_dialog.as_mut()
            && !d.show(ctx, &self.backend, &self.configs)
        {
            self.export_dialog = None;
        }
        if let Some(d) = self.import_dialog.as_mut()
            && !d.show(ctx, &self.backend, &self.configs)
        {
            self.import_dialog = None;
        }
        if let Some(d) = self.confirm.as_mut() {
            match d.show(ctx) {
                Some(true) => {
                    if let Some(c) = self.confirm.take() {
                        self.run_confirmed(c.kind);
                    }
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }
    }
}

pub enum ConfirmKind {
    Drop(ObjRef),
    Truncate(ObjRef),
    DeleteConn(String),
    DeleteFile(std::path::PathBuf),
}

trait StrongIf {
    fn strong_if(self, b: bool) -> Self;
}
impl StrongIf for RichText {
    fn strong_if(self, b: bool) -> Self {
        if b { self.strong() } else { self }
    }
}

/// A small frameless × for tabs and notices.
pub fn close_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    icons::paint(ui.painter(), rect.shrink(2.0), G::Close, ui.visuals().weak_text_color());
    resp
}

/// What the tree shows: the search text and "only favorites".
struct TreeFilter<'a> {
    text: String,
    fav_only: bool,
    favs: &'a [store::Favorite],
    /// Bumped by "Comprimi tutto" (the groups start closed again).
    generation: u64,
}

impl TreeFilter<'_> {
    fn active(&self) -> bool {
        !self.text.is_empty() || self.fav_only
    }

    fn keeps(&self, obj: &ObjRef) -> bool {
        (self.text.is_empty() || obj.name.to_lowercase().contains(&self.text)) && (!self.fav_only || is_favorite(self.favs, obj))
    }
}

/// The frame of a selected toolbar tile.
fn selected_frame(ui: &egui::Ui, r: &egui::Response) {
    ui.painter().rect_stroke(r.rect.shrink(1.0), 4.0, egui::Stroke::new(1.5, ui.visuals().selection.bg_fill), egui::StrokeKind::Inside);
}

struct TreeRow {
    resp: egui::Response,
    toggled: bool,
}

fn tree_row(ui: &mut egui::Ui, depth: usize, open: Option<bool>, icon: Ico, badge: Option<Color32>, label: &str, selected: bool) -> TreeRow {
    let h = 20.0;
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let x0 = rect.left() + 4.0 + depth as f32 * 16.0;
    let tri = egui::Rect::from_center_size(egui::pos2(x0 + 6.0, rect.center().y), egui::vec2(14.0, h));
    let toggled = open.is_some() && resp.clicked() && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| tri.contains(p));
    if ui.is_rect_visible(rect) {
        let visuals = ui.visuals();
        if selected {
            ui.painter().rect_filled(rect, 2.0, visuals.selection.bg_fill);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 2.0, visuals.widgets.hovered.weak_bg_fill);
        }
        let text_color = if selected { visuals.selection.stroke.color } else { visuals.text_color() };
        if let Some(o) = open {
            ui.painter().text(tri.center(), egui::Align2::CENTER_CENTER, if o { "⏷" } else { "⏵" }, egui::FontId::proportional(10.0), visuals.weak_text_color());
        }
        let icon_rect = egui::Rect::from_center_size(egui::pos2(x0 + 24.0, rect.center().y), egui::vec2(16.0, 16.0));
        icons::paint(ui.painter(), icon_rect, icon, text_color);
        if let Some(b) = badge {
            ui.painter().circle_filled(icon_rect.left_bottom() + egui::vec2(2.0, -2.0), 3.2, b);
            ui.painter().circle_stroke(icon_rect.left_bottom() + egui::vec2(2.0, -2.0), 3.2, egui::Stroke::new(1.0, visuals.panel_fill));
        }
        ui.painter().text(egui::pos2(x0 + 37.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), text_color);
    }
    TreeRow { resp, toggled }
}

#[allow(clippy::too_many_arguments)]
fn objects_tree(
    ui: &mut egui::Ui,
    depth: usize,
    objects: &Load<Vec<SchemaObject>>,
    loc: &Location,
    selected: Option<&ObjRef>,
    select: &mut Option<ObjRef>,
    open: &mut Option<ObjRef>,
    actions: &mut Vec<Action>,
    filter: &TreeFilter<'_>,
) {
    match objects {
        Load::Loading => {
            ui.horizontal(|ui| {
                ui.add_space(16.0 * depth as f32 + 20.0);
                ui.spinner();
            });
        }
        Load::Failed(e) => {
            ui.horizontal(|ui| {
                ui.add_space(16.0 * depth as f32 + 20.0);
                ui.colored_label(Color32::RED, e.as_str());
            });
        }
        Load::NotLoaded => {}
        Load::Ready(objs) => {
            for kind in [ObjectKind::Table, ObjectKind::View, ObjectKind::Function, ObjectKind::Procedure, ObjectKind::Trigger] {
                let make = |o: &SchemaObject| ObjRef { conn: loc.conn.clone(), db: loc.db.clone(), schema: loc.schema.clone(), name: o.name.clone(), kind: o.kind };
                let items: Vec<&SchemaObject> = objs.iter().filter(|o| o.kind == kind && filter.keeps(&make(o))).collect();
                if items.is_empty() && (filter.active() || !matches!(kind, ObjectKind::Table | ObjectKind::View)) {
                    continue;
                }
                let id = ui.make_persistent_id(("grp", loc, kind, filter.generation));
                let mut is_open = ui.data_mut(|d| *d.get_persisted_mut_or(id, kind == ObjectKind::Table)) || filter.active();
                let r = tree_row(ui, depth, Some(is_open), icon(kind).into(), None, &format!("{} ({})", i18n::t(kind.label()), items.len()), false);
                if r.toggled || r.resp.double_clicked() {
                    is_open = !is_open;
                    ui.data_mut(|d| d.insert_persisted(id, is_open));
                }
                if kind == ObjectKind::Table {
                    r.resp.context_menu(|ui| {
                        if ui.button(i18n::t("Nuova tabella")).clicked() {
                            actions.push(Action::NewTable(loc.clone()));
                        }
                        if ui.button(i18n::t("Aggiorna")).clicked() {
                            actions.push(Action::Refresh(loc.clone()));
                        }
                    });
                }
                if !is_open {
                    continue;
                }
                for o in items {
                    let obj = ObjRef { conn: loc.conn.clone(), db: loc.db.clone(), schema: loc.schema.clone(), name: o.name.clone(), kind: o.kind };
                    let fav = is_favorite(filter.favs, &obj);
                    let r = tree_row(ui, depth + 1, None, icon(kind).into(), None, &o.name, selected == Some(&obj));
                    if fav {
                        let star = egui::Rect::from_center_size(egui::pos2(r.resp.rect.right() - 12.0, r.resp.rect.center().y), egui::vec2(11.0, 11.0));
                        icons::paint(ui.painter(), star, Db::Star, ui.visuals().text_color());
                    }
                    if r.resp.clicked() {
                        *select = Some(obj.clone());
                    }
                    if r.resp.double_clicked() {
                        *open = Some(obj.clone());
                    }
                    objects::object_menu(&r.resp, &obj, fav, actions);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        while let Ok(msg) = self.backend.rx.try_recv() {
            self.handle(msg);
        }
        self.automation();
        self.cli.tick(&ctx);
        if self.cli.icon_gallery {
            egui::CentralPanel::default().show(ui, crate::icons::gallery);
            return;
        }

        self.updater.tick(&ctx);
        for (text, error) in std::mem::take(&mut self.updater.notices) {
            self.notify(text, error);
        }
        self.run_due_jobs(&ctx);
        self.ai_tick(&ctx);
        egui::Panel::top("menubar").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        if self.updater.banner(ui) && self.updater.restart() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        egui::Panel::bottom("status").exact_size(24.0).show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tree").resizable(true).default_size(250.0).min_size(160.0).show(ui, |ui| self.tree_ui(ui));
        self.chat_pane(ui, frame);
        egui::CentralPanel::default().show(ui, |ui| {
            self.tab_strip(ui);
            ui.add_space(2.0);
            let loc = self.loc.clone();
            let objects = loc.as_ref().and_then(|l| self.objects_at(l));
            let objects_snapshot: Option<Result<Vec<SchemaObject>, String>> = objects.and_then(|o| match o {
                Load::Ready(v) => Some(Ok(v.clone())),
                Load::Failed(e) => Some(Err(e.clone())),
                _ => None,
            });
            let loading = objects.is_some_and(|o| matches!(o, Load::Loading));
            let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut self.actions };
            let active = self.active.min(self.tabs.len().saturating_sub(1));
            if let Some(entry) = self.tabs.get_mut(active) {
                let id = entry.id;
                match &mut entry.tab {
                    Tab::Objects if self.objects_queries => objects::queries_ui(ui, loc.as_ref(), &mut self.objects_filter, &mut self.query_sel, &mut env),
                    Tab::Objects => {
                        let mut sel = self.selected.clone();
                        objects::ui(ui, loc.as_ref(), objects_snapshot.as_ref(), loading, &mut self.objects_filter, &self.objects_kinds, &mut sel, &self.favorites, &mut env);
                        self.selected = sel;
                    }
                    Tab::Catalog(t) => t.ui(ui, &mut env),
                    Tab::Backup(t) => t.ui(ui, &mut env),
                    Tab::Automation(t) => t.ui(ui, &mut env, &mut self.jobs, &self.running_jobs, loc.as_ref()),
                    Tab::Model(t) => t.ui(ui, &mut env),
                    Tab::AiSettings => {
                        let r = ui.available_rect_before_wrap();
                        ui.allocate_rect(r, egui::Sense::hover());
                        if !self.embeds.has(crate::embed::Slot::Settings) {
                            let msg = self.embeds.failed.clone().map(|e| i18n::tf(format!("Pagina non disponibile: {e}"))).unwrap_or_else(|| i18n::t("Apro…").to_string());
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(13.0), ui.visuals().weak_text_color());
                        }
                        self.settings_rect = Some(r);
                    }
                    Tab::Table(t) => t.ui(ui, &mut env),
                    Tab::Query(t) => t.ui(ui, &mut env),
                    Tab::Designer(t) => t.ui(ui, &mut env, id),
                }
            }
        });

        self.dialogs(&ctx);
        self.about(&ctx);
        self.notices_ui(&ctx);
        self.apply_actions();
        self.place_pages(&ctx, frame);
    }
}

/// macOS: the app's native menu bar, as in OpenSlate. The «Edit» menu is what makes Cmd+C, Cmd+V
/// and Cmd+A work in the embedded pages (AI chat, AI settings): a WebView gets copy and paste
/// from the menu, not from the keys.
#[cfg(target_os = "macos")]
fn mac_menu() -> Option<muda::Menu> {
    use muda::{Menu, PredefinedMenuItem, Submenu};
    let menu = Menu::new();
    let app = Submenu::new("DbCraft", true);
    let edit = Submenu::new(i18n::t("Modifica"), true);
    app.append_items(&[
        &PredefinedMenuItem::about(Some(i18n::t("Informazioni su DbCraft")), None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        // not the system «Quit» (it just kills the app): this one closes the window
        &muda::MenuItem::with_id(
            "quit",
            i18n::t("Esci da DbCraft"),
            true,
            Some(muda::accelerator::Accelerator::new(Some(muda::accelerator::Modifiers::SUPER), muda::accelerator::Code::KeyQ)),
        ),
    ])
    .ok()?;
    edit.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ])
    .ok()?;
    let window = Submenu::new(i18n::t("Finestra"), true);
    window.append_items(&[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::maximize(None), &PredefinedMenuItem::fullscreen(None)]).ok()?;
    menu.append_items(&[&app, &edit, &window]).ok()?;
    menu.init_for_nsapp();
    Some(menu)
}
