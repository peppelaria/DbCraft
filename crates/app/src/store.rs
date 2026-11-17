//! Saved connections (JSON in the config dir) and passwords (OS keychain).

use std::path::PathBuf;

use db_core::ConnectionConfig;

const SERVICE: &str = "dbcraft";
/// Id prefix of connections that are never saved.
pub const TRANSIENT: &str = "transient:";

fn path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DBCRAFT_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("connections.json"));
    }
    dirs::config_dir().map(|d| d.join("dbcraft").join("connections.json"))
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
