//! Saved connections (JSON in the config dir) and passwords (OS keychain).

use std::path::PathBuf;

use db_core::ConnectionConfig;

const SERVICE: &str = "dbcraft";
/// Id prefix of connections that are never saved.
pub const TRANSIENT: &str = "transient:";

/// DbCraft's config folder (connections, settings, downloaded updates).
pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DBCRAFT_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    dirs::config_dir().map(|d| d.join("dbcraft"))
}

fn path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("connections.json"))
}

/// App settings (`settings.json` in the config folder).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Tell when a newer version is on GitHub.
    pub update_check: bool,
    /// Download and install it without asking (used from the next start).
    pub update_auto: bool,
    /// A version the user chose to skip.
    pub update_skip: String,
    /// Interface language: `auto` (the system's) or a code (it, en, es, fr, de, uk, zh, ja, ko).
    pub ui_lang: String,
    /// Size of text and controls compared with egui's default (Visualizza › Dimensione…).
    pub ui_zoom: f32,
}

/// The interface sizes on offer, and the default one.
pub const UI_ZOOMS: [f32; 9] = [0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.5, 1.75, 2.0];
pub const UI_ZOOM_DEFAULT: f32 = 1.1;

impl Default for Settings {
    fn default() -> Self {
        Settings { update_check: true, update_auto: false, update_skip: String::new(), ui_lang: "auto".into(), ui_zoom: UI_ZOOM_DEFAULT }
    }
}

impl Settings {
    pub fn load() -> Settings {
        config_dir()
            .and_then(|d| std::fs::read_to_string(d.join("settings.json")).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(dir) = config_dir() else { return };
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("settings.json"), text);
        }
    }
}

pub fn load() -> Vec<ConnectionConfig> {
    let Some(p) = path() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(p) else { return Vec::new() };
    let mut v: Vec<ConnectionConfig> = serde_json::from_str(&text).unwrap_or_default();
    let mut vault = VAULT.lock().unwrap_or_else(|e| e.into_inner());
    let vault = vault.get_or_insert_with(|| Vault::open(&OsKeychain, &legacy_entries(&v)));
    for c in &mut v {
        vault.fill(c);
    }
    v
}

/// Stores or forgets the database, SSH and HTTP secrets according to the "save" checkboxes.
pub fn save_secrets(c: &ConnectionConfig) -> Result<(), String> {
    if c.id.starts_with(TRANSIENT) {
        return Ok(());
    }
    with_vault(|v| {
        v.update(c);
        v.write(&OsKeychain)
    })
}

/// Removes every secret of a connection.
pub fn delete_secrets(id: &str) {
    with_vault(|v| {
        v.remove(id);
        let _ = v.write(&OsKeychain);
    });
    // and the entries an older version may have left
    for key in secret_keys(id) {
        OsKeychain.delete(&key);
    }
}

// ----- connection secrets in the keychain -----
//
// All the secrets of all the connections live in ONE keychain entry (service "dbcraft", account
// "secrets"): a JSON object `{key: secret}`, keyed as in `secret_keys`. The keychain asks for
// permission once per entry an app reads, and on macOS an ad-hoc-signed build counts as a new
// app after every update: with one entry per secret that was a password prompt for every saved
// connection (two with an SSH tunnel) at each start; now it is a single one.
//
// Older versions kept one entry per secret (service "dbcraft", account = the key). The first
// time, they are copied into the vault and left where they are, so an older DbCraft still
// finds them.

/// Account of the keychain entry holding every secret.
const VAULT_ENTRY: &str = "secrets";

static VAULT: std::sync::Mutex<Option<Vault>> = std::sync::Mutex::new(None);

/// Keys of a connection's secrets: database password, SSH password or key passphrase, HTTP
/// tunnel password, tunnel token, proxy password.
fn secret_keys(id: &str) -> [String; 5] {
    [id.to_string(), format!("{id}:ssh"), format!("{id}:http"), format!("{id}:http-token"), format!("{id}:proxy")]
}

/// The per-secret entries an older version saved for these connections (those `load` read).
fn legacy_entries(conns: &[ConnectionConfig]) -> Vec<String> {
    let mut keys = Vec::new();
    for c in conns {
        let [db, ssh, http, token, proxy] = secret_keys(&c.id);
        if c.save_password {
            keys.push(db);
        }
        if c.ssh.save_password {
            keys.push(ssh);
        }
        if c.http.save_password {
            keys.extend([http, token, proxy]);
        }
    }
    keys
}

fn with_vault<R>(f: impl FnOnce(&mut Vault) -> R) -> R {
    let mut vault = VAULT.lock().unwrap_or_else(|e| e.into_inner());
    f(vault.get_or_insert_with(|| Vault::open(&OsKeychain, &[])))
}

/// Where the entries are kept: the OS keychain, or memory in the tests.
trait Keychain {
    /// `Ok(None)`: there is no such entry.
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn delete(&self, key: &str);
}

struct OsKeychain;

impl Keychain for OsKeychain {
    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match keyring::Entry::new(SERVICE, key).and_then(|e| e.get_password()) {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, key).and_then(|e| e.set_password(value)).map_err(|e| e.to_string())
    }

    fn delete(&self, key: &str) {
        if let Ok(e) = keyring::Entry::new(SERVICE, key) {
            let _ = e.delete_credential();
        }
    }
}

#[derive(Debug, Default)]
struct Vault {
    secrets: std::collections::BTreeMap<String, String>,
    /// Why the vault entry could not be read (access refused, damaged): then it is never
    /// written, not to overwrite the secrets in it.
    error: Option<String>,
}

impl Vault {
    /// Reads the vault entry and copies into it the older per-secret `legacy` entries it lacks.
    ///
    /// The copy is done key by key, at every start, for the keys still missing: an old entry that
    /// could not be read (access refused) is tried again next time, even if the vault has been
    /// written in the meantime (a connection saved, another old entry copied). A missing old entry
    /// costs a lookup but no prompt.
    fn open(kc: &dyn Keychain, legacy: &[String]) -> Vault {
        let mut v = match kc.get(VAULT_ENTRY) {
            Ok(Some(text)) => match serde_json::from_str(&text) {
                Ok(secrets) => Vault { secrets, error: None },
                Err(e) => return Vault { error: Some(format!("keychain entry \"{VAULT_ENTRY}\" is damaged: {e}")), ..Vault::default() },
            },
            Ok(None) => Vault::default(),
            Err(e) => return Vault { error: Some(e), ..Vault::default() },
        };
        let mut copied = false;
        for key in legacy {
            if v.secrets.contains_key(key) {
                continue;
            }
            if let Ok(Some(s)) = kc.get(key)
                && !s.is_empty()
            {
                v.secrets.insert(key.clone(), s);
                copied = true;
            }
        }
        if copied {
            let _ = v.write(kc);
        }
        v
    }

    fn write(&self, kc: &dyn Keychain) -> Result<(), String> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        kc.set(VAULT_ENTRY, &serde_json::to_string(&self.secrets).map_err(|e| e.to_string())?)
    }

    /// Puts the saved secrets into a connection read from `connections.json`.
    fn fill(&self, c: &mut ConnectionConfig) {
        let get = |key: &String| self.secrets.get(key).cloned().unwrap_or_default();
        let [db, ssh, http, token, proxy] = secret_keys(&c.id);
        if c.save_password {
            c.password = get(&db);
        }
        if c.ssh.save_password {
            c.ssh.password = get(&ssh);
        }
        if c.http.save_password {
            c.http.password = get(&http);
            c.http.token = get(&token);
            c.http.proxy_password = get(&proxy);
        }
    }

    /// Keeps the secrets of a connection whose "save" box is ticked, forgets the others.
    fn update(&mut self, c: &ConnectionConfig) {
        let http = c.http.enabled && c.http.save_password;
        let keep = [c.save_password, c.ssh.enabled && c.ssh.save_password, http, http, http];
        let values = [&c.password, &c.ssh.password, &c.http.password, &c.http.token, &c.http.proxy_password];
        for ((key, keep), value) in secret_keys(&c.id).into_iter().zip(keep).zip(values) {
            if keep && !value.is_empty() {
                self.secrets.insert(key, value.clone());
            } else {
                self.secrets.remove(&key);
            }
        }
    }

    fn remove(&mut self, id: &str) {
        for key in secret_keys(id) {
            self.secrets.remove(&key);
        }
    }
}

pub fn save(conns: &[ConnectionConfig]) -> Result<(), String> {
    let p = path().ok_or("cartella di configurazione non trovata")?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let conns: Vec<&ConnectionConfig> = conns.iter().filter(|c| !c.id.starts_with(TRANSIENT)).collect();
    let text = serde_json::to_string_pretty(&conns).map_err(|e| e.to_string())?;
    std::fs::write(p, text).map_err(|e| e.to_string())
}

// ----- favorites, saved queries, backups, scheduled jobs -----

/// A favorite object (★ in the tree, "Preferiti" menu): `favorites.json`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Favorite {
    pub conn: String,
    pub db: String,
    pub schema: Option<String>,
    pub name: String,
    pub kind: db_core::ObjectKind,
}

fn read_json<T: serde::de::DeserializeOwned + Default>(file: &str) -> T {
    config_dir().and_then(|d| std::fs::read_to_string(d.join(file)).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write_json<T: serde::Serialize>(file: &str, v: &T) -> Result<(), String> {
    let dir = config_dir().ok_or("cartella di configurazione non trovata")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(file), text).map_err(|e| e.to_string())
}

pub fn load_favorites() -> Vec<Favorite> {
    read_json("favorites.json")
}

pub fn save_favorites(f: &[Favorite]) -> Result<(), String> {
    write_json("favorites.json", &f)
}

/// A file-name-safe version of a name (connection id, database, schema).
fn safe(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_alphanumeric() || "-_.@ ".contains(c) { c } else { '_' }).collect();
    if s.is_empty() || s.starts_with('.') { format!("_{s}") } else { s }
}

/// The folder for `kind` ("queries", "backups") of a connection/database/schema.
pub fn place_dir(kind: &str, conn: &str, db: &str, schema: Option<&str>) -> PathBuf {
    let mut p = config_dir().unwrap_or_else(std::env::temp_dir).join(kind).join(safe(conn)).join(safe(if db.is_empty() { "_" } else { db }));
    if let Some(s) = schema {
        p = p.join(safe(s));
    }
    p
}

/// A file in a folder, for the lists (saved queries, backups).
#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub modified: String,
    pub size: u64,
}

/// The files with extension `ext` in `dir`, newest first.
pub fn list_files(dir: &std::path::Path, ext: &str) -> Vec<FileEntry> {
    let mut v: Vec<(std::time::SystemTime, FileEntry)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == ext))
        .filter_map(|e| {
            let md = e.metadata().ok()?;
            let t = md.modified().ok()?;
            let path = e.path();
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let modified = chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string();
            Some((t, FileEntry { path, name, modified, size: md.len() }))
        })
        .collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    v.into_iter().map(|x| x.1).collect()
}

/// What a scheduled job does.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum JobAction {
    /// A backup of the database (schema) into its backup folder.
    Backup,
    /// Runs SQL.
    Query { sql: String },
}

/// When a scheduled job runs.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Schedule {
    /// Every `n` minutes.
    Every { minutes: u32 },
    /// Every day at a time.
    Daily { hour: u32, minute: u32 },
    /// Every week, on a day (0 = Monday) at a time.
    Weekly { weekday: u32, hour: u32, minute: u32 },
}

impl Schedule {
    /// The first run after `from` (seconds since 1970).
    pub fn next_after(&self, from: i64) -> i64 {
        use chrono::{Datelike, Local, TimeZone, Timelike};
        match *self {
            Schedule::Every { minutes } => from + i64::from(minutes.max(1)) * 60,
            Schedule::Daily { hour, minute } | Schedule::Weekly { hour, minute, .. } => {
                let Some(now) = Local.timestamp_opt(from, 0).single() else { return from + 86_400 };
                for days in 0..=8 {
                    let day = now.date_naive() + chrono::Days::new(days);
                    if let Schedule::Weekly { weekday, .. } = *self
                        && day.weekday().num_days_from_monday() != weekday
                    {
                        continue;
                    }
                    let Some(t) = day.and_hms_opt(hour.min(23), minute.min(59), 0) else { continue };
                    if let Some(t) = Local.from_local_datetime(&t).earliest()
                        && t.timestamp() > from
                        && (t.hour(), t.minute()) == (hour.min(23), minute.min(59))
                    {
                        return t.timestamp();
                    }
                }
                from + 86_400
            }
        }
    }

    pub fn describe(&self) -> String {
        const DAYS: [&str; 7] = ["lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato", "domenica"];
        match *self {
            Schedule::Every { minutes: 60 } => "ogni ora".into(),
            Schedule::Every { minutes } if minutes % 60 == 0 => format!("ogni {} ore", minutes / 60),
            Schedule::Every { minutes } => format!("ogni {minutes} minuti"),
            Schedule::Daily { hour, minute } => format!("ogni giorno alle {hour:02}:{minute:02}"),
            Schedule::Weekly { weekday, hour, minute } => format!("ogni {} alle {hour:02}:{minute:02}", DAYS.get(weekday as usize).copied().unwrap_or("lunedì")),
        }
    }
}

/// A scheduled job (Automazione): `automation.json`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub conn: String,
    pub db: String,
    pub schema: Option<String>,
    pub action: JobAction,
    pub schedule: Schedule,
    /// Next run (seconds since 1970).
    pub next_run: i64,
    pub last_run: Option<i64>,
    /// Outcome of the last run (`Err` text starts with "Errore").
    pub last_result: Option<String>,
}

pub fn load_jobs() -> Vec<Job> {
    read_json("automation.json")
}

pub fn save_jobs(j: &[Job]) -> Result<(), String> {
    write_json("automation.json", &j)
}

pub fn now() -> i64 {
    chrono::Local::now().timestamp()
}

pub fn fmt_time(t: i64) -> String {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(t, 0).single().map(|t| t.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};

    /// A keychain in memory: `refused` entries fail as when the user denies access; `reads`
    /// records every entry read.
    #[derive(Default)]
    struct MemKeychain {
        items: RefCell<BTreeMap<String, String>>,
        refused: BTreeSet<String>,
        reads: RefCell<Vec<String>>,
    }

    impl MemKeychain {
        fn with(items: &[(&str, &str)]) -> MemKeychain {
            let kc = MemKeychain::default();
            kc.items.borrow_mut().extend(items.iter().map(|(k, v)| (k.to_string(), v.to_string())));
            kc
        }
        fn vault(&self) -> BTreeMap<String, String> {
            serde_json::from_str(&self.items.borrow()[VAULT_ENTRY]).unwrap()
        }
    }

    impl Keychain for MemKeychain {
        fn get(&self, key: &str) -> Result<Option<String>, String> {
            self.reads.borrow_mut().push(key.to_string());
            if self.refused.contains(key) {
                return Err("refused".into());
            }
            Ok(self.items.borrow().get(key).cloned())
        }
        fn set(&self, key: &str, value: &str) -> Result<(), String> {
            self.items.borrow_mut().insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete(&self, key: &str) {
            self.items.borrow_mut().remove(key);
        }
    }

    fn conn(id: &str, ssh: bool) -> ConnectionConfig {
        let mut c = ConnectionConfig { id: id.into(), save_password: true, ..Default::default() };
        c.ssh.enabled = ssh;
        c.ssh.save_password = ssh;
        c
    }

    #[test]
    fn legacy_entries_follow_the_save_boxes() {
        let mut http = conn("c", false);
        http.http.save_password = true;
        let keys = legacy_entries(&[conn("a", false), conn("b", true), http]);
        assert_eq!(keys, ["a", "b", "b:ssh", "c", "c:http", "c:http-token", "c:proxy"]);
    }

    #[test]
    fn old_entries_are_copied_once_and_kept() {
        let kc = MemKeychain::with(&[("a", "pa"), ("b", "pb"), ("b:ssh", "kb")]);
        let conns = [conn("a", false), conn("b", true), conn("c", true)];
        let v = Vault::open(&kc, &legacy_entries(&conns));
        let expected: BTreeMap<_, _> = [("a", "pa"), ("b", "pb"), ("b:ssh", "kb")].map(|(k, v)| (k.to_string(), v.to_string())).into();
        assert_eq!(v.secrets, expected);
        assert_eq!(kc.vault(), expected);
        // the old entries stay, for an older version
        assert_eq!(kc.items.borrow()["a"], "pa");

        // next start: the copied entries are not read again (only those still missing, which don't prompt)
        kc.reads.borrow_mut().clear();
        let mut c = conn("b", true);
        Vault::open(&kc, &legacy_entries(&conns)).fill(&mut c);
        assert_eq!(*kc.reads.borrow(), [VAULT_ENTRY, "c", "c:ssh"]);
        assert_eq!((c.password.as_str(), c.ssh.password.as_str()), ("pb", "kb"));
    }

    #[test]
    fn a_refused_old_entry_is_tried_again() {
        let mut kc = MemKeychain::with(&[("a", "pa"), ("b", "pb")]);
        kc.refused.insert("b".into());
        let legacy = ["a".to_string(), "b".to_string()];
        let mut v = Vault::open(&kc, &legacy);
        assert_eq!(v.secrets.len(), 1);
        // a connection saved meanwhile writes the vault: "b" must still be copied later
        v.update(&ConnectionConfig { password: "pc".into(), ..conn("c", false) });
        v.write(&kc).unwrap();
        kc.refused.clear();
        let v = Vault::open(&kc, &legacy);
        assert_eq!(v.secrets.get("b").map(String::as_str), Some("pb"));
        assert_eq!(kc.vault().len(), 3);
    }

    #[test]
    fn an_unreadable_vault_is_never_overwritten() {
        let mut kc = MemKeychain::with(&[(VAULT_ENTRY, r#"{"a":"pa"}"#)]);
        kc.refused.insert(VAULT_ENTRY.into());
        let mut v = Vault::open(&kc, &[]);
        v.update(&ConnectionConfig { password: "new".into(), ..conn("b", false) });
        assert!(v.write(&kc).is_err());
        assert_eq!(kc.items.borrow()[VAULT_ENTRY], r#"{"a":"pa"}"#);

        let kc = MemKeychain::with(&[(VAULT_ENTRY, "not json")]);
        assert!(Vault::open(&kc, &[]).write(&kc).is_err());
        assert_eq!(kc.items.borrow()[VAULT_ENTRY], "not json");
    }

    #[test]
    fn update_and_remove() {
        let kc = MemKeychain::default();
        let mut v = Vault::open(&kc, &[]);
        let mut c = conn("a", true);
        c.password = "pa".into();
        c.ssh.password = "ka".into();
        c.http.password = "ignored: tunnel off".into();
        v.update(&c);
        v.update(&ConnectionConfig { password: "pb".into(), ..conn("b", false) });
        v.write(&kc).unwrap();
        assert_eq!(kc.vault().keys().collect::<Vec<_>>(), ["a", "a:ssh", "b"]);

        // unticking "save" or emptying a password forgets it
        c.ssh.save_password = false;
        c.password.clear();
        v.update(&c);
        assert!(v.secrets.keys().all(|k| k == "b"));

        v.remove("b");
        v.write(&kc).unwrap();
        assert!(kc.vault().is_empty());
    }
}
