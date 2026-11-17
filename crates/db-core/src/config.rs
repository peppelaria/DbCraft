use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DbKind {
    #[default]
    Sqlite,
    Postgres,
    MySql,
    MariaDb,
    SqlServer,
}

impl DbKind {
    pub const ALL: [DbKind; 5] = [DbKind::MySql, DbKind::MariaDb, DbKind::Postgres, DbKind::SqlServer, DbKind::Sqlite];
    pub fn label(self) -> &'static str {
        match self {
            DbKind::Sqlite => "SQLite",
            DbKind::Postgres => "PostgreSQL",
            DbKind::MySql => "MySQL",
            DbKind::MariaDb => "MariaDB",
            DbKind::SqlServer => "SQL Server",
        }
    }
    pub fn default_port(self) -> u16 {
        match self {
            DbKind::Sqlite => 0,
            DbKind::Postgres => 5432,
            DbKind::MySql | DbKind::MariaDb => 3306,
            DbKind::SqlServer => 1433,
        }
    }
    pub fn default_user(self) -> &'static str {
        match self {
            DbKind::Sqlite => "",
            DbKind::Postgres => "postgres",
            DbKind::MySql | DbKind::MariaDb => "root",
            DbKind::SqlServer => "sa",
        }
    }
}

/// A saved connection. The password is not serialised: it lives in the OS keychain.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub kind: DbKind,
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Initial database (SQLite: the file path).
    pub database: String,
    #[serde(default)]
    pub ssl: bool,
    #[serde(default)]
    pub save_password: bool,
    #[serde(skip)]
    pub password: String,
    /// Optional SSH tunnel to reach the server.
    #[serde(default)]
    pub ssh: SshConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SshAuth {
    #[default]
    Password,
    /// Private key file, optionally protected by a passphrase.
    PublicKey,
}

/// SSH tunnel settings. The password (or the key passphrase) is not serialised: it lives in the
/// OS keychain.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct SshConfig {
    pub enabled: bool,
    pub host: String,
    /// 0 means 22.
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    /// Private key path (public-key authentication).
    pub key_path: String,
    pub save_password: bool,
    /// Password, or the private key's passphrase.
    #[serde(skip)]
    pub password: String,
}

impl SshConfig {
    pub fn port_or_default(&self) -> u16 {
        if self.port == 0 { 22 } else { self.port }
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = s.get(i + 1..i + 3)
            && let Ok(v) = u8::from_str_radix(h, 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl ConnectionConfig {
    /// Parses `scheme://[user[:password]@]host[:port][/database]` (`sqlite://path` or
    /// `sqlite::memory:` for SQLite). Schemes: postgres(ql), mysql, mariadb, sqlserver/mssql, sqlite.
    pub fn from_url(url: &str) -> Result<ConnectionConfig, String> {
        let (scheme, rest) = url.split_once("://").or_else(|| url.split_once(':')).ok_or("URL senza schema")?;
        let kind = match scheme.to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" | "pg" => DbKind::Postgres,
            "mysql" => DbKind::MySql,
            "mariadb" => DbKind::MariaDb,
            "sqlserver" | "mssql" => DbKind::SqlServer,
            "sqlite" => DbKind::Sqlite,
            other => return Err(format!("schema non supportato: {other}")),
        };
        let mut cfg = ConnectionConfig { kind, port: kind.default_port(), user: kind.default_user().into(), ..Default::default() };
        if kind == DbKind::Sqlite {
            cfg.database = percent_decode(rest);
            cfg.name = std::path::Path::new(&cfg.database).file_name().map_or_else(|| cfg.database.clone(), |n| n.to_string_lossy().into_owned());
            return Ok(cfg);
        }
        let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
        let rest = rest.split('#').next().unwrap_or("");
        let (auth, hostpart) = match rest.rsplit_once('@') {
            Some((a, h)) => (Some(a), h),
            None => (None, rest),
        };
        if let Some(a) = auth {
            let (u, p) = a.split_once(':').unwrap_or((a, ""));
            cfg.user = percent_decode(u);
            cfg.password = percent_decode(p);
        }
        let (hostport, db) = hostpart.split_once('/').unwrap_or((hostpart, ""));
        cfg.database = percent_decode(db);
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) if !h.ends_with(']') || h.starts_with('[') => (h, Some(p)),
            _ => (hostport, None),
        };
        cfg.host = host.trim_start_matches('[').trim_end_matches(']').to_string();
        if let Some(p) = port {
            cfg.port = p.parse().map_err(|_| format!("porta non valida: {p}"))?;
        }
        if cfg.host.is_empty() {
            cfg.host = "localhost".into();
        }
        cfg.name = format!("{} {}{}", kind.label(), cfg.host, if cfg.database.is_empty() { String::new() } else { format!("/{}", cfg.database) });
        // SSH tunnel: `?ssh=user[:password]@host[:port]`, optionally `&ssh_key=path` (the password is
        // then the key's passphrase).
        for pair in query.split('#').next().unwrap_or("").split('&').filter(|p| !p.is_empty()) {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            match k {
                "ssh" => {
                    let v = percent_decode(v);
                    let (auth, hostport) = v.rsplit_once('@').ok_or("ssh: atteso utente@host")?;
                    let (user, pw) = auth.split_once(':').unwrap_or((auth, ""));
                    let (host, port) = hostport.rsplit_once(':').map_or((hostport, "22"), |(h, p)| (h, p));
                    cfg.ssh.enabled = true;
                    cfg.ssh.user = user.to_string();
                    cfg.ssh.password = pw.to_string();
                    cfg.ssh.host = host.to_string();
                    cfg.ssh.port = port.parse().map_err(|_| format!("ssh: porta non valida: {port}"))?;
                }
                "ssh_key" => {
                    cfg.ssh.auth = SshAuth::PublicKey;
                    cfg.ssh.key_path = percent_decode(v);
                }
                _ => {}
            }
        }
        if cfg.ssh.enabled {
            cfg.name.push_str(&format!(" (SSH {})", cfg.ssh.host));
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn urls() {
        let c = ConnectionConfig::from_url("postgres://bob:p%40ss@db.local:6543/sales").unwrap();
        assert_eq!((c.kind, c.user.as_str(), c.password.as_str(), c.host.as_str(), c.port, c.database.as_str()), (DbKind::Postgres, "bob", "p@ss", "db.local", 6543, "sales"));
        let c = ConnectionConfig::from_url("mysql://127.0.0.1").unwrap();
        assert_eq!((c.user.as_str(), c.port, c.database.as_str()), ("root", 3306, ""));
        let c = ConnectionConfig::from_url("sqlserver://sa:x@[::1]:1433/master").unwrap();
        assert_eq!((c.host.as_str(), c.port), ("::1", 1433));
        let c = ConnectionConfig::from_url("sqlite:///tmp/a.db").unwrap();
        assert_eq!(c.database, "/tmp/a.db");
        assert!(ConnectionConfig::from_url("oracle://x").is_err());
        let c = ConnectionConfig::from_url("postgres://u:p@localhost/db?ssh=me:s3cr%40t@bastion.example:2222").unwrap();
        assert!(c.ssh.enabled);
        assert_eq!((c.ssh.user.as_str(), c.ssh.password.as_str(), c.ssh.host.as_str(), c.ssh.port, c.ssh.auth), ("me", "s3cr@t", "bastion.example", 2222, SshAuth::Password));
        assert_eq!((c.host.as_str(), c.database.as_str()), ("localhost", "db"));
        let c = ConnectionConfig::from_url("mysql://root@db?ssh=me@bastion&ssh_key=~/.ssh/id_ed25519").unwrap();
        assert_eq!((c.ssh.port, c.ssh.auth, c.ssh.key_path.as_str()), (22, SshAuth::PublicKey, "~/.ssh/id_ed25519"));
    }
}
