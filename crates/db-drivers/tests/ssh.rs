//! SSH tunnel tests against the `ssh` service of `docker-compose.test.yml` (user `tunnel`).
//! Skipped unless `DBCRAFT_TEST_SERVERS=1`. Generates its own throwaway keys in
//! `target/ssh-test` and authorises them on the server.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Command;

use db_core::*;

const PASSWORD: &str = "DbCraft_test1";
const PASSPHRASE: &str = "dbcraft-passphrase";

fn enabled() -> bool {
    std::env::var("DBCRAFT_TEST_SERVERS").is_ok_and(|v| v == "1")
}

fn key_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ssh-test")
}

/// Ed25519 key with a passphrase and RSA key without one, both authorised.
fn ensure_keys() -> (PathBuf, PathBuf) {
    let dir = key_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let ed = dir.join("id_ed25519_test");
    let rsa = dir.join("id_rsa_test");
    for (path, args) in [(&ed, vec!["-t", "ed25519", "-N", PASSPHRASE]), (&rsa, vec!["-t", "rsa", "-b", "3072", "-N", ""])] {
        if !path.exists() {
            let ok = Command::new("ssh-keygen").args(&args).args(["-q", "-C", "dbcraft-test", "-f"]).arg(path).status().unwrap();
            assert!(ok.success(), "ssh-keygen failed");
        }
    }
    let mut auth = String::new();
    for k in [&ed, &rsa] {
        auth.push_str(&std::fs::read_to_string(k.with_extension("pub")).unwrap());
    }
    std::fs::write(dir.join("authorized_keys"), auth).unwrap();
    (ed, rsa)
}

fn ssh(auth: SshAuth, key: Option<&PathBuf>, secret: &str) -> SshConfig {
    SshConfig {
        enabled: true,
        host: "127.0.0.1".into(),
        port: 52222,
        user: "tunnel".into(),
        auth,
        key_path: key.map(|k| k.to_string_lossy().into_owned()).unwrap_or_default(),
        save_password: false,
        password: secret.into(),
    }
}

/// The database as seen from the SSH server.
fn db(kind: DbKind, port: u16, database: &str, ssh: SshConfig) -> ConnectionConfig {
    ConnectionConfig {
        id: format!("ssh-{kind:?}"),
        name: "via ssh".into(),
        kind,
        host: "host.docker.internal".into(),
        port,
        user: kind.default_user().into(),
        password: PASSWORD.into(),
        database: database.into(),
        ssh,
        ..Default::default()
    }
}

async fn select_one(cfg: &ConnectionConfig) {
    let drv = db_drivers::connect(cfg, None).await.unwrap_or_else(|e| panic!("{e}"));
    let rs = drv.execute("SELECT 41 + 1 AS answer").await.unwrap();
    assert_eq!(rs[0].rows[0][0].as_i64(), Some(42));
    assert!(!drv.list_databases().await.unwrap().is_empty());
    // Several pooled connections share the tunnel.
    let (a, b, c) = tokio::join!(drv.execute("SELECT 1"), drv.execute("SELECT 2"), drv.execute("SELECT 3"));
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    drv.close().await;
}

#[tokio::test]
async fn postgres_via_ssh_password() {
    if !enabled() {
        return;
    }
    select_one(&db(DbKind::Postgres, 55432, "dbcraft", ssh(SshAuth::Password, None, PASSWORD))).await;
}

#[tokio::test]
async fn mysql_via_ssh_key_with_passphrase() {
    if !enabled() {
        return;
    }
    let (ed, _) = ensure_keys();
    select_one(&db(DbKind::MySql, 53306, "dbcraft", ssh(SshAuth::PublicKey, Some(&ed), PASSPHRASE))).await;
}

#[tokio::test]
async fn sqlserver_via_ssh_rsa_key() {
    if !enabled() {
        return;
    }
    let (_, rsa) = ensure_keys();
    select_one(&db(DbKind::SqlServer, 51433, "master", ssh(SshAuth::PublicKey, Some(&rsa), ""))).await;
}

#[tokio::test]
async fn ssh_errors_are_reported() {
    if !enabled() {
        return;
    }
    let (ed, _) = ensure_keys();
    let msg = |r: DbResult<std::sync::Arc<dyn Driver>>| match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    };
    let e = msg(db_drivers::connect(&db(DbKind::Postgres, 55432, "dbcraft", ssh(SshAuth::Password, None, "wrong")), None).await);
    assert!(e.contains("autenticazione rifiutata"), "{e}");
    let e = msg(db_drivers::connect(&db(DbKind::Postgres, 55432, "dbcraft", ssh(SshAuth::PublicKey, Some(&ed), "")), None).await);
    assert!(e.contains("passphrase"), "{e}");
    let e = msg(db_drivers::connect(&db(DbKind::Postgres, 55432, "dbcraft", ssh(SshAuth::PublicKey, Some(&key_dir().join("missing")), "")), None).await);
    assert!(e.contains("missing"), "{e}");
    let mut bad = ssh(SshAuth::Password, None, PASSWORD);
    bad.port = 1;
    let e = msg(db_drivers::connect(&db(DbKind::Postgres, 55432, "dbcraft", bad), None).await);
    assert!(e.starts_with("connessione: SSH"), "{e}");
}
