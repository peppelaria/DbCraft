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
    for c in &mut v {
        if c.save_password {
            c.password = get_password(&c.id).unwrap_or_default();
        }
        if c.ssh.save_password {
            c.ssh.password = get_password(&ssh_entry(&c.id)).unwrap_or_default();
        }
        if c.http.save_password {
            c.http.password = get_password(&format!("{}:http", c.id)).unwrap_or_default();
            c.http.token = get_password(&format!("{}:http-token", c.id)).unwrap_or_default();
            c.http.proxy_password = get_password(&format!("{}:proxy", c.id)).unwrap_or_default();
        }
    }
    v
}

/// Keychain entry of a connection's SSH password or key passphrase.
fn ssh_entry(id: &str) -> String {
    format!("{id}:ssh")
}

/// Stores or forgets the database and SSH secrets according to the "save" checkboxes.
pub fn save_secrets(c: &ConnectionConfig) -> Result<(), String> {
    if c.id.starts_with(TRANSIENT) {
        return Ok(());
    }
    if c.save_password {
        set_password(&c.id, &c.password)?;
    } else {
        delete_password(&c.id);
    }
    if c.ssh.enabled && c.ssh.save_password {
        set_password(&ssh_entry(&c.id), &c.ssh.password)?;
    } else {
        delete_password(&ssh_entry(&c.id));
    }
    for (suffix, secret) in [("http", &c.http.password), ("http-token", &c.http.token), ("proxy", &c.http.proxy_password)] {
        let entry = format!("{}:{suffix}", c.id);
        if c.http.enabled && c.http.save_password {
            set_password(&entry, secret)?;
        } else {
            delete_password(&entry);
        }
    }
    Ok(())
}

/// Removes every secret of a connection.
pub fn delete_secrets(id: &str) {
    delete_password(id);
    delete_password(&ssh_entry(id));
    for suffix in ["http", "http-token", "proxy"] {
        delete_password(&format!("{id}:{suffix}"));
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

pub fn get_password(id: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, id).ok()?.get_password().ok()
}

pub fn set_password(id: &str, password: &str) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, id).map_err(|e| e.to_string())?;
    if password.is_empty() {
        let _ = e.delete_credential();
        Ok(())
    } else {
        e.set_password(password).map_err(|e| e.to_string())
    }
}

pub fn delete_password(id: &str) {
    if let Ok(e) = keyring::Entry::new(SERVICE, id) {
        let _ = e.delete_credential();
    }
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
