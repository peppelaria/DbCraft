//! The main window: toolbar, connection tree, tab strip, status bar and dialogs.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use db_core::{ConnectionConfig, DbKind, Dialect, ObjectKind, SchemaObject};
use egui::{Color32, RichText};

use crate::backend::{Backend, Msg, TabId};
use crate::dialogs::{ConfirmDialog, ConnDialog, ExportDialog, ExportSource, ImportDialog};
use crate::store;
use crate::tabs::{designer::DesignerTab, objects, query::QueryTab, table::TableTab};

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
    Export(ExportSource),
    Import(ObjRef),
    Refresh(Location),
    Drop(ObjRef),
    Truncate(ObjRef),
    Notice(String, bool),
    SetTitle(TabId, String),
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
    actions: Vec<Action>,
    cli: crate::cli::Cli,
}

pub fn icon(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Table => "▦",
        ObjectKind::View => "👁",
        ObjectKind::Function => "ƒ",
        ObjectKind::Procedure => "⚙",
        ObjectKind::Trigger => "⚡",
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
            actions: Vec::new(),
            cli: cli_opts,
        };
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
                app.conn_dialog = Some(if app.cli.ssh_tab { d.on_ssh_tab() } else { d });
            }
        }
        Ok(app)
    }

    fn cfg(&self, id: &str) -> Option<&ConnectionConfig> {
        self.configs.iter().find(|c| c.id == id)
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notices.push((text.into(), error, Instant::now()));
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
            other => {
                let tab = match &other {
                    Msg::TableData { tab, .. } | Msg::Applied { tab, .. } | Msg::QueryDone { tab, .. } | Msg::DesignLoaded { tab, .. } | Msg::DesignSaved { tab, .. } => *tab,
                    _ => return,
                };
                let mut env = Env { backend: &self.backend, configs: &self.configs, actions: &mut self.actions };
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == tab) {
                    match &mut t.tab {
                        Tab::Table(t) => t.handle(other, &mut env),
                        Tab::Query(t) => t.handle(other, &mut env),
                        Tab::Designer(t) => t.handle(other, &mut env),
                        Tab::Objects => {}
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
                    let title = format!("{} @{}", obj.name, if obj.db.is_empty() { cfg.name.clone() } else { obj.db.clone() });
                    let id = self.next_id;
                    let mut tab = TableTab::new(id, obj);
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
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let conn_btn = big_button(ui, "🔌", "Connessione");
            egui::Popup::menu(&conn_btn).show(|ui| {
                for k in DbKind::ALL {
                    if ui.button(RichText::new(format!("●  {}…", k.label())).color(kind_color(k))).clicked() {
                        self.conn_dialog = Some(ConnDialog::new(k));
                    }
                }
            });
            ui.separator();
            let loc = self.loc.clone();
            if big_button(ui, "📝", "Nuova query").clicked() {
                match loc.clone() {
                    Some(l) => self.actions.push(Action::NewQuery(l)),
                    None => self.notify("Apri prima una connessione/database", true),
                }
            }
            if big_button(ui, "▦", "Tabella").clicked() {
                self.active = 0;
            }
            if big_button(ui, "➕", "Nuova tabella").clicked() {
                match loc.clone() {
                    Some(l) => self.actions.push(Action::NewTable(l)),
                    None => self.notify("Apri prima un database", true),
                }
            }
            ui.separator();
            let sel_table = self.selected.clone().filter(|o| o.kind == ObjectKind::Table);
            if big_button(ui, "📥", "Importa").clicked() {
                match sel_table.clone() {
                    Some(o) => self.actions.push(Action::Import(o)),
                    None => self.notify("Seleziona una tabella da importare", true),
                }
            }
            if big_button(ui, "📤", "Esporta").clicked() {
                match self.selected.clone().filter(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View)) {
                    Some(o) => self.actions.push(Action::Export(ExportSource::Table(o))),
                    None => self.notify("Seleziona una tabella o vista da esportare", true),
                }
            }
            ui.separator();
            if big_button(ui, "🔄", "Aggiorna").clicked()
                && let Some(l) = loc
            {
                self.actions.push(Action::Refresh(l));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dark = ui.visuals().dark_mode;
                if ui.button(if dark { "☀" } else { "🌙" }).on_hover_text("Tema chiaro/scuro").clicked() {
                    ui.ctx().set_theme(if dark { egui::Theme::Light } else { egui::Theme::Dark });
                }
            });
        });
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

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            if self.configs.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("Nessuna connessione.\nUsa «Connessione» nella barra in alto.").weak());
            }
            for cfg in &self.configs {
                let node = self.tree.entry(cfg.id.clone()).or_default();
                let connected = matches!(node.dbs, Some(Load::Ready(_)));
                let loading = matches!(node.dbs, Some(Load::Loading));
                let r = tree_row(ui, 0, Some(node.open), RichText::new("●").color(if connected { kind_color(cfg.kind) } else { Color32::GRAY }), &cfg.name, false);
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
                        if ui.button("Chiudi connessione").clicked() {
                            disconnect = Some(cfg.id.clone());
                        }
                    } else if ui.button("Apri connessione").clicked() {
                        connect = Some(cfg.id.clone());
                    }
                    if ui.button("Modifica connessione…").clicked() {
                        edit = Some(cfg.id.clone());
                    }
                    if ui.button("Elimina connessione").clicked() {
                        delete = Some(cfg.id.clone());
                    }
                    if connected && ui.button("Nuova query").clicked() {
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
                    let r = tree_row(ui, 1, Some(db.open), RichText::new("🗄").color(if active { Color32::from_rgb(0xD9, 0xA4, 0x06) } else { Color32::GRAY }), &db.name, false);
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
                        if ui.button("Nuova query").clicked() {
                            self.actions.push(Action::NewQuery(here.clone()));
                        }
                        if db.schemas.ready().is_some_and(Vec::is_empty) {
                            if ui.button("Nuova tabella").clicked() {
                                self.actions.push(Action::NewTable(here.clone()));
                            }
                            if ui.button("Aggiorna").clicked() {
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
                            objects_tree(ui, 2, &db.objects, &here, self.selected.as_ref(), &mut select_obj, &mut open_obj, &mut self.actions);
                        }
                        Load::Ready(schemas) => {
                            for s in schemas.iter_mut() {
                                let sl = Location { conn: cfg.id.clone(), db: db.name.clone(), schema: Some(s.name.clone()) };
                                let r = tree_row(ui, 2, Some(s.open), RichText::new("📂"), &s.name, self.loc.as_ref() == Some(&sl));
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
                                    if ui.button("Nuova query").clicked() {
                                        self.actions.push(Action::NewQuery(sl.clone()));
                                    }
                                    if ui.button("Nuova tabella").clicked() {
                                        self.actions.push(Action::NewTable(sl.clone()));
                                    }
                                    if ui.button("Aggiorna").clicked() {
                                        load_objects = Some(sl.clone());
                                    }
                                });
                                if s.open {
                                    objects_tree(ui, 3, &s.objects, &sl, self.selected.as_ref(), &mut select_obj, &mut open_obj, &mut self.actions);
                                }
                            }
                        }
                        Load::NotLoaded => {}
                    }
                }
            }
        });

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
                    let glyph = match &t.tab {
                        Tab::Objects => "☰",
                        Tab::Table(_) => "▦",
                        Tab::Query(_) => "📝",
                        Tab::Designer(_) => "🖊",
                    };
                    let frame = egui::Frame::new()
                        .fill(if selected { ui.visuals().extreme_bg_color } else { ui.visuals().faint_bg_color })
                        .inner_margin(egui::Margin::symmetric(10, 5))
                        .stroke(egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let label = ui.add(egui::Label::new(RichText::new(format!("{glyph} {}", t.title)).strong_if(selected)).sense(egui::Sense::click()));
                            if label.clicked() {
                                self.active = i;
                            }
                            if label.middle_clicked() && i > 0 {
                                close = Some(i);
                            }
                            if i > 0 && ui.add(egui::Button::new(RichText::new("×").small()).frame(false)).clicked() {
                                close = Some(i);
                            }
                        });
                    });
                }
            });
        });
        if let Some(i) = close {
            let dirty = match &self.tabs[i].tab {
                Tab::Table(t) => !t.edits.is_empty(),
                _ => false,
            };
            if dirty {
                self.notify("La tabella ha modifiche non salvate: applicale o annullale prima di chiudere", true);
            } else {
                self.tabs.remove(i);
                if self.active >= i {
                    self.active = self.active.saturating_sub(1);
                }
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(l) = &self.loc {
                let name = self.cfg(&l.conn).map(|c| c.name.clone()).unwrap_or_default();
                let mut s = format!("🔌 {name}");
                if !l.db.is_empty() {
                    s.push_str(&format!("  ›  🗄 {}", l.db));
                }
                if let Some(sc) = &l.schema {
                    s.push_str(&format!("  ›  📂 {sc}"));
                }
                ui.label(RichText::new(s).small());
            }
            if let Some(TabEntry { tab: Tab::Table(t), .. }) = self.tabs.get(self.active) {
                ui.separator();
                ui.label(RichText::new(t.status_text()).small());
            }
            if let Some(TabEntry { tab: Tab::Query(t), .. }) = self.tabs.get(self.active) {
                ui.separator();
                ui.label(RichText::new(t.status_text()).small());
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
                        ui.label(RichText::new(if *error { "⚠" } else { "✔" }).color(Color32::BLACK));
                        ui.add(egui::Label::new(RichText::new(text).color(Color32::BLACK)).wrap());
                        if ui.small_button("×").clicked() {
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
        if let Some(sql) = self.cli.query.take() {
            let id = self.next_id;
            let mut tab = QueryTab::new(id, loc);
            tab.set_sql(sql, true);
            self.add_tab("Query 1".into(), Tab::Query(Box::new(tab)));
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
}

trait StrongIf {
    fn strong_if(self, b: bool) -> Self;
}
impl StrongIf for RichText {
    fn strong_if(self, b: bool) -> Self {
        if b { self.strong() } else { self }
    }
}

fn big_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(74.0, 50.0), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.style().interact(&resp);
        if resp.hovered() || resp.is_pointer_button_down_on() {
            ui.painter().rect_filled(rect, 4.0, v.bg_fill);
        }
        let color = ui.visuals().text_color();
        ui.painter().text(rect.center_top() + egui::vec2(0.0, 16.0), egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(20.0), color);
        ui.painter().text(rect.center_bottom() - egui::vec2(0.0, 10.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), color);
    }
    resp
}

struct TreeRow {
    resp: egui::Response,
    toggled: bool,
}

fn tree_row(ui: &mut egui::Ui, depth: usize, open: Option<bool>, icon: RichText, label: &str, selected: bool) -> TreeRow {
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
        let icon_pos = egui::pos2(x0 + 22.0, rect.center().y);
        let galley = egui::WidgetText::from(icon.size(13.0)).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body);
        ui.painter().galley(icon_pos - egui::vec2(galley.size().x / 2.0, galley.size().y / 2.0), galley, text_color);
        ui.painter().text(egui::pos2(x0 + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(13.0), text_color);
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
                let items: Vec<&SchemaObject> = objs.iter().filter(|o| o.kind == kind).collect();
                if items.is_empty() && !matches!(kind, ObjectKind::Table | ObjectKind::View) {
                    continue;
                }
                let id = ui.make_persistent_id(("grp", loc, kind));
                let mut is_open = ui.data_mut(|d| *d.get_persisted_mut_or(id, kind == ObjectKind::Table));
                let r = tree_row(ui, depth, Some(is_open), RichText::new(icon(kind)), &format!("{} ({})", kind.label(), items.len()), false);
                if r.toggled || r.resp.double_clicked() {
                    is_open = !is_open;
                    ui.data_mut(|d| d.insert_persisted(id, is_open));
                }
                if kind == ObjectKind::Table {
                    r.resp.context_menu(|ui| {
                        if ui.button("Nuova tabella").clicked() {
                            actions.push(Action::NewTable(loc.clone()));
                        }
                        if ui.button("Aggiorna").clicked() {
                            actions.push(Action::Refresh(loc.clone()));
                        }
                    });
                }
                if !is_open {
                    continue;
                }
                for o in items {
                    let obj = ObjRef { conn: loc.conn.clone(), db: loc.db.clone(), schema: loc.schema.clone(), name: o.name.clone(), kind: o.kind };
                    let r = tree_row(ui, depth + 1, None, RichText::new(icon(kind)), &o.name, selected == Some(&obj));
                    if r.resp.clicked() {
                        *select = Some(obj.clone());
                    }
                    if r.resp.double_clicked() {
                        *open = Some(obj.clone());
                    }
                    objects::object_menu(&r.resp, &obj, actions);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        while let Ok(msg) = self.backend.rx.try_recv() {
            self.handle(msg);
        }
        self.automation();
        self.cli.tick(&ctx);

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::bottom("status").exact_size(24.0).show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tree").resizable(true).default_size(250.0).min_size(160.0).show(ui, |ui| self.tree_ui(ui));
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
                    Tab::Objects => {
                        let mut sel = self.selected.clone();
                        objects::ui(ui, loc.as_ref(), objects_snapshot.as_ref(), loading, &mut self.objects_filter, &mut sel, &mut env);
                        self.selected = sel;
                    }
                    Tab::Table(t) => t.ui(ui, &mut env),
                    Tab::Query(t) => t.ui(ui, &mut env),
                    Tab::Designer(t) => t.ui(ui, &mut env, id),
                }
            }
        });

        self.dialogs(&ctx);
        self.notices_ui(&ctx);
        self.apply_actions();
    }
}
