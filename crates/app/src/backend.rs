//! Async plumbing: a tokio runtime, a cache of open drivers and a channel back to the UI.

use std::collections::HashMap;
use std::future::Future;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;

use db_core::{ConnectionConfig, DbResult, Driver, ResultSet, SchemaObject, TableInfo};

pub type TabId = u64;

/// Results delivered to the UI thread.
pub enum Msg {
    Connected { conn: String, res: DbResult<Vec<String>> },
    Schemas { conn: String, db: String, res: DbResult<Vec<String>> },
    Objects { conn: String, db: String, schema: Option<String>, res: DbResult<Vec<SchemaObject>> },
    TableData { tab: TabId, info: Option<DbResult<TableInfo>>, page: DbResult<ResultSet>, total: Option<u64> },
    Applied { tab: TabId, res: DbResult<u64> },
    QueryDone { tab: TabId, run: u64, res: DbResult<Vec<ResultSet>> },
    /// Code completion metadata: databases, schemas and objects of a query tab's location.
    QueryMeta { tab: TabId, generation: u64, dbs: Option<DbResult<Vec<String>>>, schemas: Option<DbResult<Vec<String>>>, objects: DbResult<Vec<SchemaObject>> },
    QueryColumns { tab: TabId, generation: u64, table: String, res: DbResult<TableInfo> },
    BuilderTables { tab: TabId, res: DbResult<Vec<SchemaObject>> },
    BuilderTable { tab: TabId, name: String, res: DbResult<TableInfo> },
    DesignLoaded { tab: TabId, res: DbResult<TableInfo> },
    DesignSaved { tab: TabId, res: Result<(), String> },
    ObjectsChanged { conn: String, db: String, schema: Option<String> },
    ImportInfo { res: DbResult<TableInfo> },
    Imported { res: DbResult<u64> },
    Notice { text: String, error: bool },
}

type Key = (String, String);

pub struct Backend {
    rt: tokio::runtime::Runtime,
    drivers: Arc<tokio::sync::Mutex<HashMap<Key, Arc<dyn Driver>>>>,
    tx: Sender<Msg>,
    pub rx: Receiver<Msg>,
    ctx: egui::Context,
}

impl Backend {
    pub fn new(ctx: egui::Context) -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().thread_name("dbcraft-io").build()?;
        let (tx, rx) = channel();
        Ok(Backend { rt, drivers: Default::default(), tx, rx, ctx })
    }

    /// Runs `f` with a driver for (`cfg`, `db`), connecting on first use; `""` is the
    /// connection's default database. The handle aborts the job (see [`Backend::evict`]).
    pub fn with_driver<F, Fut>(&self, cfg: &ConnectionConfig, db: &str, f: F) -> tokio::task::AbortHandle
    where
        F: FnOnce(DbResult<Arc<dyn Driver>>) -> Fut + Send + 'static,
        Fut: Future<Output = Msg> + Send + 'static,
    {
        let cfg = cfg.clone();
        let key = (cfg.id.clone(), db.to_string());
        let drivers = self.drivers.clone();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let job = self.rt.spawn(async move {
            let drv = {
                let mut map = drivers.lock().await;
                match map.get(&key) {
                    Some(d) => Ok(d.clone()),
                    None => {
                        let db = (!key.1.is_empty()).then_some(key.1.as_str());
                        match db_drivers::connect(&cfg, db).await {
                            Ok(d) => {
                                map.insert(key.clone(), d.clone());
                                Ok(d)
                            }
                            Err(e) => Err(e),
                        }
                    }
                }
            };
            let msg = f(drv).await;
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
        job.abort_handle()
    }

    /// Forgets the cached driver of (`conn`, `db`) and closes it: after aborting a job its
    /// connection may be mid-statement. The next job connects again.
    pub fn evict(&self, conn: &str, db: &str) {
        let drivers = self.drivers.clone();
        let key = (conn.to_string(), db.to_string());
        self.rt.spawn(async move {
            let old = drivers.lock().await.remove(&key);
            if let Some(d) = old {
                d.close().await;
            }
        });
    }

    pub fn disconnect(&self, conn: &str) {
        let drivers = self.drivers.clone();
        let conn = conn.to_string();
        self.rt.spawn(async move {
            let closing: Vec<Arc<dyn Driver>> = {
                let mut map = drivers.lock().await;
                let keys: Vec<Key> = map.keys().filter(|k| k.0 == conn).cloned().collect();
                keys.iter().filter_map(|k| map.remove(k)).collect()
            };
            for d in closing {
                d.close().await;
            }
        });
    }

    /// Runs a blocking job (file export/import) off the UI thread.
    pub fn spawn<Fut>(&self, fut: Fut)
    where
        Fut: Future<Output = Msg> + Send + 'static,
    {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        self.rt.spawn(async move {
            let msg = fut.await;
            let _ = tx.send(msg);
            ctx.request_repaint();
        });
    }
}
