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
}
