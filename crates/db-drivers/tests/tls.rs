//! SSL/TLS tests against `docker-compose.test.yml`, with the certificates made by
//! `scripts/test-certs.sh` (target/tls-test). Skipped unless `DBCRAFT_TEST_SERVERS=1`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;

use db_core::*;

const PASSWORD: &str = "DbCraft_test1";

fn enabled() -> bool {
    std::env::var("DBCRAFT_TEST_SERVERS").is_ok_and(|v| v == "1")
}

fn cert(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tls-test").join(name);
    assert!(p.is_file(), "run scripts/test-certs.sh first ({})", p.display());
    p.to_string_lossy().into_owned()
}

fn cfg(kind: DbKind, port: u16, database: &str, verify: TlsVerify, ca: bool, client: bool) -> ConnectionConfig {
    ConnectionConfig {
        id: format!("tls-{kind:?}"),
        name: "tls".into(),
        kind,
        host: "127.0.0.1".into(),
        port,
        user: kind.default_user().into(),
        password: PASSWORD.into(),
        database: database.into(),
        ssl: true,
        tls: TlsConfig {
            verify,
            ca_cert: if ca { cert("ca.crt") } else { String::new() },
            client_cert: if client { cert("client.crt") } else { String::new() },
            client_key: if client { cert("client.key") } else { String::new() },
        },
        ..Default::default()
    }
}

async fn connect(c: &ConnectionConfig) -> DbResult<Arc<dyn Driver>> {
    db_drivers::connect(c, None).await
}

async fn one(d: &Arc<dyn Driver>, sql: &str) -> Value {
    let rs = d.execute(sql).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
    rs.into_iter().find(|r| !r.columns.is_empty()).and_then(|r| r.rows.into_iter().next()).and_then(|r| r.into_iter().next()).unwrap_or_default()
}

#[tokio::test]
async fn postgres_ssl_modes() {
    if !enabled() {
        return;
    }
    let ssl_on = "SELECT ssl::text FROM pg_stat_ssl WHERE pid = pg_backend_pid()";
    // Encrypted without checking the certificate.
    let d = connect(&cfg(DbKind::Postgres, 55432, "dbcraft", TlsVerify::None, false, false)).await.unwrap();
    assert_eq!(one(&d, ssl_on).await.to_string(), "true");
    // Full verification against the test CA (the certificate names 127.0.0.1).
    let d = connect(&cfg(DbKind::Postgres, 55432, "dbcraft", TlsVerify::Full, true, false)).await.unwrap();
    assert_eq!(one(&d, ssl_on).await.to_string(), "true");
    // With a client certificate, the server sees its subject.
    let d = connect(&cfg(DbKind::Postgres, 55432, "dbcraft", TlsVerify::Ca, true, true)).await.unwrap();
    assert_eq!(one(&d, "SELECT client_dn FROM pg_stat_ssl WHERE pid = pg_backend_pid()").await.to_string(), "/CN=postgres");
    // The test CA is not among the system's roots: verification without it fails.
    assert!(connect(&cfg(DbKind::Postgres, 55432, "dbcraft", TlsVerify::Ca, false, false)).await.is_err());
}

#[tokio::test]
async fn mysql_ssl_modes_and_x509_user() {
    if !enabled() {
        return;
    }
    let cipher = "SELECT VARIABLE_VALUE FROM performance_schema.session_status WHERE VARIABLE_NAME = 'Ssl_cipher'";
    let d = connect(&cfg(DbKind::MySql, 53306, "dbcraft", TlsVerify::Full, true, false)).await.unwrap();
    assert!(!one(&d, cipher).await.to_string().is_empty(), "not encrypted");
    // A user that must present a client certificate.
    let _ = d.execute("DROP USER IF EXISTS 'x509user'@'%'").await;
    d.execute(&format!("CREATE USER 'x509user'@'%' IDENTIFIED BY '{PASSWORD}' REQUIRE X509")).await.unwrap();
    let mut without = cfg(DbKind::MySql, 53306, "", TlsVerify::Ca, true, false);
    without.user = "x509user".into();
    assert!(connect(&without).await.is_err(), "REQUIRE X509 must refuse a connection without a client certificate");
    let mut with = cfg(DbKind::MySql, 53306, "", TlsVerify::Ca, true, true);
    with.user = "x509user".into();
    let x = connect(&with).await.unwrap();
    assert_eq!(one(&x, "SELECT CURRENT_USER()").await.to_string(), "x509user@%");
    d.execute("DROP USER 'x509user'@'%'").await.unwrap();
}

#[tokio::test]
async fn sqlserver_encryption() {
    if !enabled() {
        return;
    }
    let d = connect(&cfg(DbKind::SqlServer, 51433, "master", TlsVerify::None, false, false)).await.unwrap();
    assert_eq!(one(&d, "SELECT encrypt_option FROM sys.dm_exec_connections WHERE session_id = @@SPID").await.to_string(), "TRUE");
    // Its certificate is self-signed: verifying it against the system's roots fails.
    assert!(connect(&cfg(DbKind::SqlServer, 51433, "master", TlsVerify::Ca, false, false)).await.is_err());
}

#[tokio::test]
async fn missing_files_are_reported() {
    let mut c = cfg(DbKind::Postgres, 55432, "dbcraft", TlsVerify::Ca, false, false);
    c.tls.ca_cert = "/no/such/ca.pem".into();
    let e = match connect(&c).await {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("certificato CA non trovato"), "{e}");
    c.tls.ca_cert.clear();
    c.tls.client_cert = "/tmp".into();
    let e = match connect(&c).await {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("certificato client non trovato") || e.contains("servono sia"), "{e}");
}
