//! Lists that come from the server's catalog: the "Utente" toolbar button (users and roles, with
//! new user, password, privileges, delete) and the "Altri" menu (indexes, sequences, events,
//! types, extensions, synonyms).

use db_core::{DbKind, DbResult, Dialect, ResultSet, Value};
use egui::RichText;

use crate::app::{Action, Env, Location};
use crate::backend::{Msg, TabId};
use crate::icons::{self, Db, G};
use crate::i18n;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogKind {
    Users,
    Indexes,
    Sequences,
    Events,
    Types,
    Extensions,
    Synonyms,
}

impl CatalogKind {
    pub fn label(self) -> &'static str {
        match self {
            CatalogKind::Users => "Utenti",
            CatalogKind::Indexes => "Indici",
            CatalogKind::Sequences => "Sequenze",
            CatalogKind::Events => "Eventi",
            CatalogKind::Types => "Tipi",
            CatalogKind::Extensions => "Estensioni",
            CatalogKind::Synonyms => "Sinonimi",
        }
    }

    pub fn icon(self) -> icons::Ico {
        match self {
            CatalogKind::Users => Db::User.into(),
            CatalogKind::Indexes => Db::Index.into(),
            CatalogKind::Sequences => Db::Key.into(),
            CatalogKind::Events => G::Clock.into(),
            CatalogKind::Types => Db::Column.into(),
            CatalogKind::Extensions => Db::Others.into(),
            CatalogKind::Synonyms => G::Link.into(),
        }
    }

    /// The kinds of the "Altri" menu that this server has.
    pub fn others(kind: DbKind) -> Vec<CatalogKind> {
        match kind {
            DbKind::MySql => vec![CatalogKind::Indexes, CatalogKind::Events],
            DbKind::MariaDb => vec![CatalogKind::Indexes, CatalogKind::Sequences, CatalogKind::Events],
            DbKind::Postgres => vec![CatalogKind::Indexes, CatalogKind::Sequences, CatalogKind::Types, CatalogKind::Extensions],
            DbKind::SqlServer => vec![CatalogKind::Indexes, CatalogKind::Sequences, CatalogKind::Types, CatalogKind::Synonyms],
            DbKind::Sqlite => vec![CatalogKind::Indexes],
        }
    }

    /// The query listing these objects at `loc`.
    pub fn sql(self, kind: DbKind, loc: &Location) -> Option<String> {
        let d = Dialect::from(kind);
        let db = d.string_literal(&loc.db);
        let schema = d.string_literal(loc.schema.as_deref().unwrap_or(if kind == DbKind::SqlServer { "dbo" } else { "public" }));
        let my = matches!(kind, DbKind::MySql | DbKind::MariaDb);
        Some(match (self, kind) {
            (CatalogKind::Users, _) if my => "SELECT User AS utente, Host AS host FROM mysql.user ORDER BY 1, 2".into(),
            (CatalogKind::Users, DbKind::Postgres) => "SELECT rolname AS ruolo, CASE WHEN rolcanlogin THEN 'sì' ELSE 'no' END AS accesso, \
                CASE WHEN rolsuper THEN 'sì' ELSE 'no' END AS superutente, CASE WHEN rolcreatedb THEN 'sì' ELSE 'no' END AS crea_db, \
                CASE WHEN rolcreaterole THEN 'sì' ELSE 'no' END AS crea_ruoli, rolvaliduntil::text AS scadenza \
                FROM pg_roles WHERE rolname NOT LIKE 'pg\\_%' ORDER BY 1"
                .into(),
            (CatalogKind::Users, DbKind::SqlServer) => "SELECT name AS utente, type_desc AS tipo, CONVERT(varchar(19), create_date, 120) AS creato, \
                CASE WHEN is_disabled = 1 THEN 'sì' ELSE 'no' END AS disattivato, default_database_name AS database_predefinito \
                FROM sys.server_principals WHERE type IN ('S','U','G') AND name NOT LIKE '##%' ORDER BY 1"
                .into(),
            (CatalogKind::Indexes, _) if my => format!(
                "SELECT TABLE_NAME AS tabella, INDEX_NAME AS indice, GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX SEPARATOR ', ') AS colonne, \
                 IF(NON_UNIQUE = 0, 'sì', 'no') AS univoco, INDEX_TYPE AS tipo FROM information_schema.STATISTICS WHERE TABLE_SCHEMA = {db} \
                 GROUP BY TABLE_NAME, INDEX_NAME, NON_UNIQUE, INDEX_TYPE ORDER BY 1, 2"
            ),
            (CatalogKind::Indexes, DbKind::Postgres) => {
                format!("SELECT tablename AS tabella, indexname AS indice, indexdef AS definizione FROM pg_indexes WHERE schemaname = {schema} ORDER BY 1, 2")
            }
            (CatalogKind::Indexes, DbKind::SqlServer) => format!(
                "SELECT t.name AS tabella, i.name AS indice, i.type_desc AS tipo, CASE WHEN i.is_unique = 1 THEN 'sì' ELSE 'no' END AS univoco, \
                 CASE WHEN i.is_primary_key = 1 THEN 'sì' ELSE 'no' END AS chiave_primaria, \
                 STUFF((SELECT ', ' + c.name FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
                   WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id ORDER BY ic.key_ordinal FOR XML PATH('')), 1, 2, '') AS colonne \
                 FROM sys.indexes i JOIN sys.tables t ON t.object_id = i.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id \
                 WHERE i.name IS NOT NULL AND s.name = {schema} ORDER BY 1, 2"
            ),
            (CatalogKind::Indexes, DbKind::Sqlite) => {
                "SELECT tbl_name AS tabella, name AS indice, COALESCE(sql, '(automatico)') AS definizione FROM sqlite_master WHERE type = 'index' ORDER BY 1, 2".into()
            }
            (CatalogKind::Sequences, DbKind::MariaDb) => {
                format!("SELECT TABLE_NAME AS sequenza FROM information_schema.TABLES WHERE TABLE_SCHEMA = {db} AND TABLE_TYPE = 'SEQUENCE' ORDER BY 1")
            }
            (CatalogKind::Sequences, DbKind::Postgres) => format!(
                "SELECT sequencename AS sequenza, data_type::text AS tipo, start_value AS inizio, increment_by AS incremento, last_value AS ultimo_valore, \
                 max_value AS massimo, CASE WHEN cycle THEN 'sì' ELSE 'no' END AS ciclica FROM pg_sequences WHERE schemaname = {schema} ORDER BY 1"
            ),
            (CatalogKind::Sequences, DbKind::SqlServer) => format!(
                "SELECT name AS sequenza, CAST(start_value AS varchar(40)) AS inizio, CAST(increment AS varchar(40)) AS incremento, \
                 CAST(current_value AS varchar(40)) AS valore_attuale FROM sys.sequences WHERE SCHEMA_NAME(schema_id) = {schema} ORDER BY 1"
            ),
            (CatalogKind::Events, _) if my => format!(
                "SELECT EVENT_NAME AS evento, STATUS AS stato, EVENT_TYPE AS tipo, CAST(EXECUTE_AT AS CHAR) AS quando, \
                 CONCAT_WS(' ', INTERVAL_VALUE, INTERVAL_FIELD) AS ogni, CAST(STARTS AS CHAR) AS inizio, CAST(ENDS AS CHAR) AS fine, \
                 CAST(LAST_EXECUTED AS CHAR) AS ultima_esecuzione, EVENT_DEFINITION AS definizione FROM information_schema.EVENTS WHERE EVENT_SCHEMA = {db} ORDER BY 1"
            ),
            (CatalogKind::Types, DbKind::Postgres) => format!(
                "SELECT t.typname AS tipo, CASE t.typtype WHEN 'e' THEN 'enumerazione' WHEN 'c' THEN 'composito' WHEN 'd' THEN 'dominio' WHEN 'r' THEN 'intervallo' END AS genere, \
                 CASE t.typtype WHEN 'e' THEN (SELECT string_agg(e.enumlabel, ', ' ORDER BY e.enumsortorder) FROM pg_enum e WHERE e.enumtypid = t.oid) \
                   WHEN 'd' THEN format_type(t.typbasetype, t.typtypmod) END AS definizione \
                 FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace WHERE n.nspname = {schema} AND t.typtype IN ('e','c','d','r') \
                 AND (t.typtype <> 'c' OR EXISTS (SELECT 1 FROM pg_class c WHERE c.oid = t.typrelid AND c.relkind = 'c')) ORDER BY 1"
            ),
            (CatalogKind::Types, DbKind::SqlServer) => "SELECT name AS tipo, TYPE_NAME(system_type_id) AS tipo_base, max_length AS lunghezza, \
                CASE WHEN is_nullable = 1 THEN 'sì' ELSE 'no' END AS nullable FROM sys.types WHERE is_user_defined = 1 ORDER BY 1"
                .into(),
            (CatalogKind::Extensions, DbKind::Postgres) => "SELECT e.extname AS estensione, e.extversion AS versione, n.nspname AS schema, \
                obj_description(e.oid, 'pg_extension') AS descrizione FROM pg_extension e JOIN pg_namespace n ON n.oid = e.extnamespace ORDER BY 1"
                .into(),
            (CatalogKind::Synonyms, DbKind::SqlServer) => {
                format!("SELECT name AS sinonimo, base_object_name AS oggetto FROM sys.synonyms WHERE SCHEMA_NAME(schema_id) = {schema} ORDER BY 1")
            }
            _ => return None,
        })
    }
}

/// The user dialog: new user, password, privileges.
#[derive(Clone, Debug, PartialEq)]
enum UserForm {
    New { name: String, host: String, password: String, confirm: String },
    Password { password: String, confirm: String },
    Privileges { privs: Vec<(&'static str, bool)>, scope: String, grant: bool },
}

const PRIVS_MY: [&str; 12] = ["SELECT", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER", "INDEX", "REFERENCES", "CREATE VIEW", "EXECUTE", "ALL PRIVILEGES"];
const PRIVS_PG: [&str; 8] = ["SELECT", "INSERT", "UPDATE", "DELETE", "TRUNCATE", "REFERENCES", "TRIGGER", "ALL PRIVILEGES"];
const PRIVS_MS: [&str; 8] = ["SELECT", "INSERT", "UPDATE", "DELETE", "EXECUTE", "ALTER", "REFERENCES", "CONTROL"];

pub struct CatalogTab {
    pub id: TabId,
    pub loc: Location,
    pub kind: CatalogKind,
    data: Option<Result<ResultSet, String>>,
    loading: bool,
    filter: String,
    selected: Option<usize>,
    /// Users: the privileges of the selected user.
    detail: Option<Result<ResultSet, String>>,
    form: Option<UserForm>,
    confirm_drop: bool,
}

/// (user, host) of a row of the users list.
fn user_of(rs: &ResultSet, row: usize) -> Option<(String, String)> {
    let r = rs.rows.get(row)?;
    let name = r.first().map(|v| v.to_string())?;
    let host = if rs.columns.get(1).is_some_and(|c| c.name == "host") { r.get(1).map(|v| v.to_string()).unwrap_or_default() } else { String::new() };
    Some((name, host))
}

impl CatalogTab {
    pub fn new(id: TabId, loc: Location, kind: CatalogKind) -> Self {
        CatalogTab { id, loc, kind, data: None, loading: false, filter: String::new(), selected: None, detail: None, form: None, confirm_drop: false }
    }

    pub fn title(&self, conn_name: &str) -> String {
        if self.kind == CatalogKind::Users || self.loc.db.is_empty() { format!("{} @{conn_name}", i18n::t(self.kind.label())) } else { format!("{} @{}", i18n::t(self.kind.label()), self.loc.db) }
    }

    fn db_kind(&self, env: &Env<'_>) -> DbKind {
        env.cfg(&self.loc.conn).map_or(DbKind::Sqlite, |c| c.kind)
    }

    fn run(&self, env: &Env<'_>, tag: u8, sql: String) {
        let Some(cfg) = env.cfg(&self.loc.conn).cloned() else {
            return;
        };
        let tab = self.id;
        env.backend.with_driver(&cfg, &self.loc.db, move |d| async move {
            let res = match d {
                Ok(d) => d.execute(&sql).await,
                Err(e) => Err(e),
            };
            Msg::Catalog { tab, tag, res }
        });
    }

    pub fn reload(&mut self, env: &mut Env<'_>) {
        let kind = self.db_kind(env);
        match self.kind.sql(kind, &self.loc) {
            Some(sql) => {
                self.loading = true;
                self.run(env, 0, sql);
            }
            None => {
                let what = if self.kind == CatalogKind::Users {
                    "SQLite non ha utenti: l'accesso è quello al file del database.".to_string()
                } else {
                    format!("{}: non disponibili su {}", self.kind.label(), kind.label())
                };
                self.data = Some(Err(what));
            }
        }
    }

    pub fn handle(&mut self, msg: Msg, env: &mut Env<'_>) {
        let Msg::Catalog { tag, res, .. } = msg else {
            return;
        };
        let first = |r: DbResult<Vec<ResultSet>>| r.map_err(|e| e.to_string()).map(|mut v| if v.is_empty() { ResultSet::default() } else { v.remove(0) });
        match tag {
            0 => {
                self.loading = false;
                self.data = Some(first(res));
                self.selected = None;
                self.detail = None;
            }
            1 => self.detail = Some(first(res)),
            _ => match res {
                Ok(_) => {
                    env.actions.push(Action::Notice("Fatto".into(), false));
                    self.form = None;
                    let sel = self.selected;
                    self.reload(env);
                    self.selected = sel;
                }
                Err(e) => env.actions.push(Action::Notice(e.to_string(), true)),
            },
        }
    }

    fn selected_user(&self) -> Option<(String, String)> {
        let rs = self.data.as_ref()?.as_ref().ok()?;
        user_of(rs, self.selected?)
    }

    fn load_detail(&mut self, env: &Env<'_>) {
        let Some((name, host)) = self.selected_user() else {
            return;
        };
        let kind = self.db_kind(env);
        let d = Dialect::from(kind);
        let sql = match kind {
            DbKind::MySql | DbKind::MariaDb => format!("SHOW GRANTS FOR {}@{}", d.string_literal(&name), d.string_literal(&host)),
            DbKind::Postgres => format!(
                "SELECT table_schema || '.' || table_name AS oggetto, string_agg(privilege_type, ', ' ORDER BY privilege_type) AS privilegi \
                 FROM information_schema.role_table_grants WHERE grantee = {} GROUP BY 1 \
                 UNION ALL SELECT 'ruolo ' || r.rolname, 'membro' FROM pg_auth_members m JOIN pg_roles r ON r.oid = m.roleid JOIN pg_roles u ON u.oid = m.member WHERE u.rolname = {} ORDER BY 1",
                d.string_literal(&name),
                d.string_literal(&name)
            ),
            DbKind::SqlServer => format!(
                "SELECT class_desc AS ambito, COALESCE(OBJECT_NAME(major_id), SCHEMA_NAME(major_id), '') AS oggetto, permission_name AS privilegio, state_desc AS stato \
                 FROM sys.database_permissions WHERE grantee_principal_id = USER_ID({}) \
                 UNION ALL SELECT 'SERVER', '', 'ruolo ' + r.name, 'MEMBRO' FROM sys.server_role_members m JOIN sys.server_principals r ON r.principal_id = m.role_principal_id \
                 JOIN sys.server_principals u ON u.principal_id = m.member_principal_id WHERE u.name = {} ORDER BY 1, 2",
                d.string_literal(&name),
                d.string_literal(&name)
            ),
            DbKind::Sqlite => return,
        };
        self.detail = None;
        self.run(env, 1, sql);
    }

    /// The statements of the user form.
    fn form_sql(&self, kind: DbKind) -> Result<Vec<String>, String> {
        let d = Dialect::from(kind);
        let lit = |s: &str| d.string_literal(s);
        let my = matches!(kind, DbKind::MySql | DbKind::MariaDb);
        let who = |name: &str, host: &str| {
            if my { format!("{}@{}", lit(name), lit(if host.is_empty() { "%" } else { host })) } else { d.quote(name) }
        };
        let Some(form) = &self.form else {
            return Ok(Vec::new());
        };
        match form {
            UserForm::New { name, host, password, confirm } => {
                if name.trim().is_empty() {
                    return Err("Scrivi il nome dell'utente".into());
                }
                if password != confirm {
                    return Err("Le due password non coincidono".into());
                }
                let n = name.trim();
                Ok(match kind {
                    DbKind::MySql | DbKind::MariaDb => vec![format!("CREATE USER {} IDENTIFIED BY {}", who(n, host.trim()), lit(password))],
                    DbKind::Postgres => vec![format!("CREATE ROLE {} LOGIN PASSWORD {}", d.quote(n), lit(password))],
                    DbKind::SqlServer => {
                        let mut v = vec![format!("CREATE LOGIN {} WITH PASSWORD = {}", d.quote(n), lit(password))];
                        if !self.loc.db.is_empty() {
                            v.push(format!("CREATE USER {} FOR LOGIN {}", d.quote(n), d.quote(n)));
                        }
                        v
                    }
                    DbKind::Sqlite => return Err("SQLite non ha utenti".into()),
                })
            }
            UserForm::Password { password, confirm } => {
                let (name, host) = self.selected_user().ok_or("Seleziona un utente")?;
                if password != confirm {
                    return Err("Le due password non coincidono".into());
                }
                Ok(vec![match kind {
                    DbKind::MySql | DbKind::MariaDb => format!("ALTER USER {} IDENTIFIED BY {}", who(&name, &host), lit(password)),
                    DbKind::Postgres => {
                        format!("ALTER ROLE {} PASSWORD {}", d.quote(&name), lit(password))
                    }
                    DbKind::SqlServer => format!("ALTER LOGIN {} WITH PASSWORD = {}", d.quote(&name), lit(password)),
                    DbKind::Sqlite => return Err("SQLite non ha utenti".into()),
                }])
            }
            UserForm::Privileges { privs, scope, grant } => {
                let (name, host) = self.selected_user().ok_or("Seleziona un utente")?;
                let chosen: Vec<&str> = privs.iter().filter(|p| p.1).map(|p| p.0).collect();
                if chosen.is_empty() {
                    return Err("Scegli almeno un privilegio".into());
                }
                let list = if chosen.contains(&"ALL PRIVILEGES") { "ALL PRIVILEGES".to_string() } else { chosen.join(", ") };
                let (verb, prep) = if *grant { ("GRANT", "TO") } else { ("REVOKE", "FROM") };
                let scope = scope.trim();
                Ok(match kind {
                    DbKind::MySql | DbKind::MariaDb => {
                        let on = if scope.is_empty() || scope == "*" { "*.*".to_string() } else { format!("{}.*", d.quote(scope)) };
                        vec![format!("{verb} {list} ON {on} {prep} {}", who(&name, &host))]
                    }
                    DbKind::Postgres => {
                        let s = if scope.is_empty() { "public" } else { scope };
                        let mut v = vec![format!("{verb} {list} ON ALL TABLES IN SCHEMA {} {prep} {}", d.quote(s), d.quote(&name))];
                        if *grant {
                            v.insert(0, format!("GRANT USAGE ON SCHEMA {} TO {}", d.quote(s), d.quote(&name)));
                        }
                        v
                    }
                    DbKind::SqlServer => {
                        let s = if scope.is_empty() { "dbo" } else { scope };
                        let mut v = Vec::new();
                        if *grant {
                            v.push(format!("IF USER_ID({}) IS NULL CREATE USER {} FOR LOGIN {}", lit(&name), d.quote(&name), d.quote(&name)));
                        }
                        v.push(format!("{verb} {list} ON SCHEMA::{} {prep} {}", d.quote(s), d.quote(&name)));
                        v
                    }
                    DbKind::Sqlite => return Err("SQLite non ha utenti".into()),
                })
            }
        }
    }

    fn drop_sql(&self, kind: DbKind) -> Option<String> {
        let (name, host) = self.selected_user()?;
        let d = Dialect::from(kind);
        Some(match kind {
            DbKind::MySql | DbKind::MariaDb => format!("DROP USER {}@{}", d.string_literal(&name), d.string_literal(&host)),
            // its objects pass to the current user, its privileges go, then the role
            DbKind::Postgres => format!("REASSIGN OWNED BY {0} TO CURRENT_USER; DROP OWNED BY {0}; DROP ROLE {0}", d.quote(&name)),
            // the user of this database, then the login
            DbKind::SqlServer => format!("IF USER_ID({}) IS NOT NULL DROP USER {1}; DROP LOGIN {1}", d.string_literal(&name), d.quote(&name)),
            DbKind::Sqlite => return None,
        })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &mut Env<'_>) {
        if self.data.is_none() && !self.loading {
            self.reload(env);
        }
        let kind = self.db_kind(env);
        let users = self.kind == CatalogKind::Users;
        let has_sel = self.selected.is_some();
        ui.horizontal_wrapped(|ui| {
            if users {
                if icons::button_ex(ui, Db::User, "Nuovo utente", false, kind != DbKind::Sqlite).clicked() {
                    self.form = Some(UserForm::New { name: String::new(), host: "%".into(), password: String::new(), confirm: String::new() });
                }
                if icons::button_ex(ui, Db::Key, "Cambia password", false, has_sel).clicked() {
                    self.form = Some(UserForm::Password { password: String::new(), confirm: String::new() });
                }
                if icons::button_ex(ui, Db::Design, "Privilegi", false, has_sel).clicked() {
                    let list: &[&'static str] = match kind {
                        DbKind::Postgres => &PRIVS_PG,
                        DbKind::SqlServer => &PRIVS_MS,
                        _ => &PRIVS_MY,
                    };
                    let scope = match kind {
                        DbKind::Postgres | DbKind::SqlServer => self.loc.schema.clone().unwrap_or_default(),
                        _ => self.loc.db.clone(),
                    };
                    self.form = Some(UserForm::Privileges { privs: list.iter().map(|p| (*p, false)).collect(), scope, grant: true });
                }
                if icons::button_ex(ui, G::Delete, "Elimina utente", false, has_sel).clicked() {
                    self.confirm_drop = true;
                }
                ui.separator();
            }
            if icons::button(ui, Db::Refresh, "Aggiorna").clicked() {
                self.reload(env);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text(i18n::t("Cerca")).desired_width(180.0));
                icons::show(ui, G::Search, 16.0);
            });
        });
        ui.separator();
        if self.loading {
            ui.spinner();
            return;
        }
        let rs = match &self.data {
            Some(Ok(rs)) => rs.clone(),
            Some(Err(e)) => {
                ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
                return;
            }
            None => return,
        };
        let detail_h = if users && has_sel { 180.0 } else { 0.0 };
        let avail = ui.available_height() - detail_h;
        let mut clicked = None;
        ui.allocate_ui(egui::vec2(ui.available_width(), avail.max(80.0)), |ui| {
            clicked = result_table(ui, ("catalog", self.id), &rs, &self.filter, self.selected);
        });
        if let Some(r) = clicked {
            self.selected = Some(r);
            if users {
                self.load_detail(env);
            }
        }
        if users && has_sel {
            ui.separator();
            let who = self.selected_user().map(|(n, h)| if h.is_empty() { n } else { format!("{n}@{h}") }).unwrap_or_default();
            ui.label(RichText::new(i18n::tf(format!("Privilegi di {who}"))).strong());
            match &self.detail {
                Some(Ok(d)) => {
                    let d = d.clone();
                    result_table(ui, ("grants", self.id), &d, "", None);
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
                }
                None => {
                    ui.spinner();
                }
            }
        }
        self.form_ui(ui.ctx(), env, kind);
        if self.confirm_drop {
            let who = self.selected_user().map(|(n, _)| n).unwrap_or_default();
            let mut close = false;
            egui::Modal::new(egui::Id::unique(("drop_user", self.id))).show(ui.ctx(), |ui| {
                ui.label(i18n::tf(format!("Eliminare l'utente \"{who}\"? L'operazione non è reversibile.")));
                ui.horizontal(|ui| {
                    if ui.button(i18n::t("Elimina")).clicked() {
                        if let Some(sql) = self.drop_sql(kind) {
                            self.run(env, 2, sql);
                        }
                        close = true;
                    }
                    if ui.button(i18n::t("Annulla")).clicked() {
                        close = true;
                    }
                });
            });
            if close {
                self.confirm_drop = false;
            }
        }
    }

    fn form_ui(&mut self, ctx: &egui::Context, env: &mut Env<'_>, kind: DbKind) {
        if self.form.is_none() {
            return;
        }
        let sql = self.form_sql(kind);
        let my = matches!(kind, DbKind::MySql | DbKind::MariaDb);
        let mut close = false;
        let mut run = false;
        let target = self.selected_user().map(|(n, _)| n).unwrap_or_default();
        egui::Modal::new(egui::Id::unique(("user_form", self.id))).show(ctx, |ui| {
            ui.set_width(460.0);
            let Some(form) = self.form.as_mut() else {
                return;
            };
            match form {
                UserForm::New { name, host, password, confirm } => {
                    ui.heading(i18n::t("Nuovo utente"));
                    egui::Grid::new("new_user").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label(i18n::t("Nome"));
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        if my {
                            ui.label(i18n::t("Host"));
                            ui.add(egui::TextEdit::singleline(host).hint_text(i18n::t("% = da qualsiasi host")));
                            ui.end_row();
                        }
                        ui.label(i18n::t("Password"));
                        ui.add(egui::TextEdit::singleline(password).password(true));
                        ui.end_row();
                        ui.label(i18n::t("Conferma"));
                        ui.add(egui::TextEdit::singleline(confirm).password(true));
                        ui.end_row();
                    });
                }
                UserForm::Password { password, confirm } => {
                    ui.heading(i18n::tf(format!("Cambia password di {target}")));
                    egui::Grid::new("pw_user").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label(i18n::t("Nuova password"));
                        ui.add(egui::TextEdit::singleline(password).password(true));
                        ui.end_row();
                        ui.label(i18n::t("Conferma"));
                        ui.add(egui::TextEdit::singleline(confirm).password(true));
                        ui.end_row();
                    });
                }
                UserForm::Privileges { privs, scope, grant } => {
                    ui.heading(i18n::tf(format!("Privilegi di {target}")));
                    ui.horizontal(|ui| {
                        ui.radio_value(grant, true, i18n::t("Concedi (GRANT)"));
                        ui.radio_value(grant, false, i18n::t("Revoca (REVOKE)"));
                    });
                    ui.horizontal(|ui| {
                        ui.label(match kind {
                            DbKind::Postgres | DbKind::SqlServer => i18n::t("Su tutte le tabelle dello schema"),
                            _ => i18n::t("Sul database (* = tutti)"),
                        });
                        ui.text_edit_singleline(scope);
                    });
                    egui::Grid::new("privs").num_columns(3).show(ui, |ui| {
                        for (i, (p, on)) in privs.iter_mut().enumerate() {
                            ui.checkbox(on, *p);
                            if i % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
                }
            }
            ui.separator();
            ui.label(RichText::new(i18n::t("SQL")).small().weak());
            match &sql {
                Ok(v) => {
                    let mut shown = v.iter().map(|s| format!("{s};")).collect::<Vec<_>>().join("\n");
                    // the password is not shown in the preview
                    if let Some(UserForm::New { password, .. } | UserForm::Password { password, .. }) = &self.form
                        && !password.is_empty()
                    {
                        shown = shown.replace(&Dialect::from(kind).string_literal(password), "'••••••'");
                    }
                    ui.add(egui::Label::new(RichText::new(shown).monospace()).wrap());
                }
                Err(e) => {
                    ui.label(RichText::new(e.as_str()).color(ui.visuals().error_fg_color));
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(sql.is_ok(), egui::Button::new(i18n::t("Esegui"))).clicked() {
                    run = true;
                }
                if ui.button(i18n::t("Annulla")).clicked() {
                    close = true;
                }
            });
        });
        if run && let Ok(v) = sql {
            let joined = v.iter().map(|s| format!("{s};")).collect::<Vec<_>>().join("\n");
            // SQL Server: CREATE LOGIN / CREATE USER one at a time
            if kind == DbKind::SqlServer {
                for s in v {
                    self.run(env, 2, s);
                }
            } else {
                self.run(env, 2, joined);
            }
        }
        if close {
            self.form = None;
        }
    }
}

/// A read-only table of a result set; returns the clicked row (index in `rs.rows`).
pub fn result_table(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, rs: &ResultSet, filter: &str, selected: Option<usize>) -> Option<usize> {
    let needle = filter.to_lowercase();
    let shown: Vec<usize> = (0..rs.rows.len()).filter(|&i| needle.is_empty() || rs.rows[i].iter().any(|v| v.to_string().to_lowercase().contains(&needle))).collect();
    if rs.columns.is_empty() {
        ui.label(RichText::new(i18n::t("Nessun elemento")).weak());
        return None;
    }
    let mut clicked = None;
    // wide lists scroll sideways
    ui.push_id(id, |ui| egui::ScrollArea::horizontal().show(ui, |ui| {
        let mut t = egui_extras::TableBuilder::new(ui).striped(true).resizable(true).sense(egui::Sense::click()).auto_shrink([false, false]);
        for (i, _) in rs.columns.iter().enumerate() {
            t = t.column(if i + 1 == rs.columns.len() { egui_extras::Column::remainder().at_least(80.0) } else { egui_extras::Column::initial(150.0).at_least(40.0).clip(true) });
        }
        t.header(22.0, |mut h| {
            for c in &rs.columns {
                h.col(|ui| {
                    ui.strong(i18n::tr(&c.name.replace('_', " ")).into_owned());
                });
            }
        })
        .body(|body| {
            body.rows(22.0, shown.len(), |mut row| {
                let Some(&i) = shown.get(row.index()) else {
                    return;
                };
                row.set_selected(selected == Some(i));
                for v in &rs.rows[i] {
                    row.col(|ui| {
                        match v {
                            Value::Null => ui.label(RichText::new(i18n::t("(Null)")).italics().weak()),
                            // MySQL gives some catalog texts (event definitions) as bytes
                            // the yes/no of the catalog queries
                            Value::Text(t) if t == "sì" || t == "no" => ui.label(i18n::tr(t).into_owned()),
                            Value::Bytes(b) if std::str::from_utf8(b).is_ok() => ui.add(egui::Label::new(String::from_utf8_lossy(b)).truncate()),
                            v => ui.add(egui::Label::new(v.to_string()).truncate()),
                        };
                    });
                }
                if row.response().clicked() {
                    clicked = Some(i);
                }
            });
        });
    }));
    if shown.is_empty() {
        ui.label(RichText::new(i18n::t("Nessun elemento")).weak());
    }
    clicked
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use db_core::ColumnMeta;

    /// The "Altri" lists and the users list run on every server; a user is created, gets a new
    /// password and privileges, is shown and dropped.
    #[test]
    fn catalogs_and_users_on_servers() {
        if !crate::servertest::enabled() {
            return;
        }
        let rt = tokio::runtime::Runtime::new().unwrap();
        let tmp = std::env::temp_dir().join(format!("dbcraft-catalog-test-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        for kind in crate::servertest::ALL {
            rt.block_on(async {
                let (d, schema, cfg) = crate::servertest::scratch(kind, "cat_t", &tmp).await;
                let dl = d.dialect();
                let t = dl.qualified(schema.as_deref(), "items");
                d.execute(&format!("CREATE TABLE {t} (id INT PRIMARY KEY, name VARCHAR(50))")).await.unwrap();
                d.execute(&format!("CREATE INDEX ix_items_name ON {t} (name)")).await.unwrap();
                let db = match kind {
                    DbKind::MySql | DbKind::MariaDb => "cat_t".to_string(),
                    DbKind::SqlServer => "dbcraft".into(),
                    _ => cfg.database.clone(),
                };
                let loc = Location { conn: cfg.id.clone(), db, schema: schema.clone() };
                let mut kinds = CatalogKind::others(kind);
                kinds.push(CatalogKind::Users);
                for c in kinds {
                    let Some(sql) = c.sql(kind, &loc) else {
                        assert!(c == CatalogKind::Users && kind == DbKind::Sqlite, "{kind:?} {c:?}: no SQL");
                        continue;
                    };
                    let rs = d.execute(&sql).await.unwrap_or_else(|e| panic!("{kind:?} {c:?}: {e}\n{sql}"));
                    if c == CatalogKind::Indexes {
                        let found = rs[0].rows.iter().any(|r| r.iter().any(|v| v.to_string().contains("ix_items_name")));
                        assert!(found, "{kind:?}: index not listed");
                    }
                    if c == CatalogKind::Users {
                        assert!(!rs[0].rows.is_empty(), "{kind:?}: no users");
                    }
                }
                if kind == DbKind::Sqlite {
                    return;
                }
                // a user: create, password, privileges, detail, revoke, drop
                let name = "dbcraft_u1";
                let mut tab = CatalogTab::new(1, loc.clone(), CatalogKind::Users);
                let host_col = matches!(kind, DbKind::MySql | DbKind::MariaDb);
                let mut cols = vec![ColumnMeta { name: "utente".into(), type_name: String::new() }];
                let mut row = vec![Value::Text(name.into())];
                if host_col {
                    cols.push(ColumnMeta { name: "host".into(), type_name: String::new() });
                    row.push(Value::Text("%".into()));
                }
                tab.data = Some(Ok(ResultSet { columns: cols, rows: vec![row], ..Default::default() }));
                tab.selected = Some(0);
                if let Some(sql) = tab.drop_sql(kind) {
                    let _ = d.execute(&sql).await;
                }
                let run = |v: Vec<String>| {
                    let d = d.clone();
                    async move {
                        for s in v {
                            d.execute(&s).await.unwrap_or_else(|e| panic!("{kind:?}: {s}: {e}"));
                        }
                    }
                };
                tab.form = Some(UserForm::New { name: name.into(), host: "%".into(), password: "Pw_12345678!".into(), confirm: "Pw_12345678!".into() });
                run(tab.form_sql(kind).unwrap()).await;
                tab.form = Some(UserForm::Password { password: "Pw_87654321!".into(), confirm: "Pw_87654321!".into() });
                run(tab.form_sql(kind).unwrap()).await;
                let list: &[&'static str] = match kind {
                    DbKind::Postgres => &PRIVS_PG,
                    DbKind::SqlServer => &PRIVS_MS,
                    _ => &PRIVS_MY,
                };
                let scope = match kind {
                    DbKind::Postgres | DbKind::SqlServer => schema.clone().unwrap_or_default(),
                    _ => "cat_t".into(),
                };
                let privs: Vec<(&'static str, bool)> = list.iter().map(|p| (*p, matches!(*p, "SELECT" | "INSERT"))).collect();
                tab.form = Some(UserForm::Privileges { privs: privs.clone(), scope: scope.clone(), grant: true });
                run(tab.form_sql(kind).unwrap()).await;
                tab.form = Some(UserForm::Privileges { privs, scope, grant: false });
                run(tab.form_sql(kind).unwrap()).await;
                // the detail query of load_detail
                let detail = match kind {
                    DbKind::MySql | DbKind::MariaDb => format!("SHOW GRANTS FOR '{name}'@'%'"),
                    DbKind::Postgres => format!("SELECT privilege_type FROM information_schema.role_table_grants WHERE grantee = '{name}'"),
                    _ => format!("SELECT permission_name FROM sys.database_permissions WHERE grantee_principal_id = USER_ID('{name}')"),
                };
                d.execute(&detail).await.unwrap();
                run(vec![tab.drop_sql(kind).unwrap()]).await;
                d.close().await;
            });
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
