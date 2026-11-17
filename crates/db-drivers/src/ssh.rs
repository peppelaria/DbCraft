//! SSH tunnel: a local TCP listener on 127.0.0.1 whose connections are forwarded through an SSH
//! session (`direct-tcpip` channels) to the database host as seen from the SSH server.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use db_core::{DbError, DbResult, Dialect, Driver, PageRequest, ResultSet, RowChange, SchemaObject, SshAuth, SshConfig, TableInfo};
use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

fn err(e: impl std::fmt::Display) -> DbError {
    DbError::Connection(format!("SSH: {e}"))
}

struct Client {
    host: String,
    port: u16,
}

impl client::Handler for Client {
    type Error = russh::Error;

    /// Known hosts: a key that differs from the one recorded in `~/.ssh/known_hosts` is refused
    /// (possible man-in-the-middle). Unknown hosts are accepted, as a first connection is.
    async fn check_server_key(&mut self, key: &russh::keys::PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        match russh::keys::check_known_hosts(&self.host, self.port, &key.public_key()) {
            Ok(_) => Ok(true),
            Err(russh::keys::Error::KeyChanged { line }) => {
                eprintln!("SSH: the host key of {}:{} differs from known_hosts line {line}", self.host, self.port);
                Ok(false)
            }
            // No known_hosts file or an unreadable one: treat the host as unknown.
            Err(_) => Ok(true),
        }
    }
}

/// An open tunnel. Dropping it stops forwarding and closes the SSH session.
pub struct SshTunnel {
    pub local_port: u16,
    accept: JoinHandle<()>,
    session: Arc<client::Handle<Client>>,
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.accept.abort();
        let session = self.session.clone();
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            rt.spawn(async move {
                let _ = session.disconnect(russh::Disconnect::ByApplication, "", "en").await;
            });
        }
    }
}

/// Connects and authenticates to the SSH server, then forwards a local port to
/// `target_host:target_port`.
pub async fn open(cfg: &SshConfig, target_host: &str, target_port: u16) -> DbResult<SshTunnel> {
    let host = cfg.host.trim().to_string();
    if host.is_empty() {
        return Err(err("host SSH mancante"));
    }
    let port = cfg.port_or_default();
    let config = Arc::new(client::Config { inactivity_timeout: None, keepalive_interval: Some(Duration::from_secs(30)), ..Default::default() });
    let handler = Client { host: host.clone(), port };
    let mut session = tokio::time::timeout(Duration::from_secs(20), client::connect(config, (host.as_str(), port), handler))
        .await
        .map_err(|_| err("timeout di connessione"))?
        .map_err(|e| match e {
            russh::Error::UnknownKey => err("la chiave dell'host è cambiata rispetto a ~/.ssh/known_hosts"),
            other => err(other),
        })?;

    let auth = match cfg.auth {
        SshAuth::Password => session.authenticate_password(cfg.user.clone(), cfg.password.clone()).await.map_err(err)?,
        SshAuth::PublicKey => {
            let path = expand_home(cfg.key_path.trim());
            let passphrase = (!cfg.password.is_empty()).then_some(cfg.password.as_str());
            let key = russh::keys::load_secret_key(&path, passphrase).map_err(|e| match e {
                russh::keys::Error::KeyIsEncrypted => err("la chiave privata è protetta: inserisci la passphrase"),
                other => err(format!("chiave privata {path}: {other}")),
            })?;
            let hash = session.best_supported_rsa_hash().await.map_err(err)?.flatten();
            session.authenticate_publickey(cfg.user.clone(), PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await.map_err(err)?
        }
    };
    if !auth.success() {
        return Err(err(format!("autenticazione rifiutata per l'utente \"{}\"", cfg.user)));
    }

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(err)?;
    let local_port = listener.local_addr().map_err(err)?.port();
    let session = Arc::new(session);
    let fwd = session.clone();
    let target_host = target_host.to_string();
    let accept = tokio::spawn(async move {
        while let Ok((mut socket, peer)) = listener.accept().await {
            let fwd = fwd.clone();
            let target_host = target_host.clone();
            tokio::spawn(async move {
                let channel = match fwd.channel_open_direct_tcpip(target_host, u32::from(target_port), peer.ip().to_string(), u32::from(peer.port())).await {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("SSH: forwarding refused: {e}");
                        return;
                    }
                };
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
            });
        }
    });
    Ok(SshTunnel { local_port, accept, session })
}

fn expand_home(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => std::path::Path::new(&home).join(rest).to_string_lossy().into_owned(),
        _ => path.to_string(),
    }
}

/// A driver that keeps its SSH tunnel alive for as long as it lives.
pub struct TunneledDriver {
    pub inner: Arc<dyn Driver>,
    pub tunnel: SshTunnel,
}

#[async_trait]
impl Driver for TunneledDriver {
    fn dialect(&self) -> Dialect {
        self.inner.dialect()
    }
    async fn list_databases(&self) -> DbResult<Vec<String>> {
        self.inner.list_databases().await
    }
    async fn list_schemas(&self) -> DbResult<Vec<String>> {
        self.inner.list_schemas().await
    }
    async fn list_objects(&self, schema: Option<&str>) -> DbResult<Vec<SchemaObject>> {
        self.inner.list_objects(schema).await
    }
    async fn describe_table(&self, schema: Option<&str>, table: &str) -> DbResult<TableInfo> {
        self.inner.describe_table(schema, table).await
    }
    async fn execute(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        self.inner.execute(sql).await
    }
    async fn explain(&self, sql: &str) -> DbResult<Vec<ResultSet>> {
        self.inner.explain(sql).await
    }
    async fn fetch_page(&self, req: &PageRequest) -> DbResult<ResultSet> {
        self.inner.fetch_page(req).await
    }
    async fn count_rows(&self, schema: Option<&str>, table: &str, filter: Option<&str>) -> DbResult<u64> {
        self.inner.count_rows(schema, table, filter).await
    }
    async fn apply_changes(&self, schema: Option<&str>, table: &str, changes: &[RowChange]) -> DbResult<u64> {
        self.inner.apply_changes(schema, table, changes).await
    }
    async fn close(&self) {
        self.inner.close().await;
    }
}
