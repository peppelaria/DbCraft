//! HTTP tunnel tests: `tunnel/dbcraft-tunnel.php` served by the `http-tunnel` service of
//! `docker-compose.test.yml`. Skipped unless `DBCRAFT_TEST_SERVERS=1`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use db_core::*;

const PASSWORD: &str = "DbCraft_test1";

fn enabled() -> bool {
    std::env::var("DBCRAFT_TEST_SERVERS").is_ok_and(|v| v == "1")
}

fn tunnel(base64: bool) -> HttpTunnelConfig {
    HttpTunnelConfig {
        enabled: true,
        url: "http://127.0.0.1:58080/dbcraft-tunnel.php".into(),
        base64,
        token: "test-token".into(),
        basic_auth: true,
        user: "tunnel".into(),
        password: PASSWORD.into(),
        ..Default::default()
    }
}

/// The database as the web server sees it.
fn cfg(kind: DbKind, port: u16, database: &str, http: HttpTunnelConfig) -> ConnectionConfig {
    ConnectionConfig {
        id: format!("http-{kind:?}"),
        name: "via http".into(),
        kind,
        host: "host.docker.internal".into(),
        port,
        user: kind.default_user().into(),
        password: PASSWORD.into(),
        database: database.into(),
        http,
        ..Default::default()
    }
}

async fn exec(d: &Arc<dyn Driver>, sql: &str) -> Vec<ResultSet> {
    d.execute(sql).await.unwrap_or_else(|e| panic!("{sql}\n→ {e}"))
}

async fn roundtrip(d: Arc<dyn Driver>) {
    let dl = d.dialect();
    let _ = d.execute("DROP TABLE IF EXISTS http_items").await;
    let _ = d.execute("DROP TABLE IF EXISTS http_groups").await;
    let (bytes_ty, bool_ty, bytes_lit, yes) = match dl {
        Dialect::Postgres => ("bytea", "boolean", "'\\x00ff'::bytea", "TRUE"),
        _ => ("blob", "tinyint(1)", "X'00FF'", "1"),
    };
    exec(&d, "CREATE TABLE http_groups (id int PRIMARY KEY, name varchar(40))").await;
    exec(
        &d,
        &format!(
            "CREATE TABLE http_items (id int PRIMARY KEY, group_id int REFERENCES http_groups(id), label varchar(80) NOT NULL, \
             price decimal(10,2), ok {bool_ty}, data {bytes_ty}{})",
            if dl == Dialect::MySql { ", FOREIGN KEY (group_id) REFERENCES http_groups(id)" } else { "" }
        ),
    )
    .await;
    // A multi-statement script with a quoted semicolon: split client-side, one result each.
    let rs = exec(
        &d,
        &format!(
            "INSERT INTO http_groups VALUES (1, 'a;b'); \
             INSERT INTO http_items VALUES (1, 1, 'Caffè ☕', 2.50, {yes}, {bytes_lit}), (2, 1, 'x', NULL, NULL, NULL); \
             SELECT id, label, price, ok, data FROM http_items ORDER BY id"
        ),
    )
    .await;
    assert_eq!(rs.len(), 3);
    assert_eq!(rs[1].affected, Some(2));
    let r = &rs[2];
    assert_eq!(r.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["id", "label", "price", "ok", "data"]);
    assert_eq!(r.rows[0][0], Value::Int(1));
    assert_eq!(r.rows[0][1], Value::Text("Caffè ☕".into()));
    assert_eq!(r.rows[0][2].to_string(), "2.50");
    assert!(matches!(r.rows[0][3], Value::Bool(true) | Value::Int(1)), "{:?}", r.rows[0][3]);
    assert_eq!(r.rows[0][4], Value::Bytes(vec![0, 255]));
    assert!(r.rows[1][2].is_null() && r.rows[1][4].is_null());

    // Introspection through the tunnel.
    let objs = d.list_objects(None).await.unwrap();
    assert!(objs.iter().any(|o| o.name == "http_items"));
    let info = d.describe_table(None, "http_items").await.unwrap();
    assert_eq!(info.primary_key(), vec!["id".to_string()]);
    assert!(info.foreign_keys.iter().any(|f| f.ref_table == "http_groups"), "{:?}", info.foreign_keys);
    assert!(d.list_databases().await.unwrap().contains(&"dbcraft".to_string()));

    // Edits in one transaction; a failing one rolls back.
    let n = d
        .apply_changes(
            None,
            "http_items",
            &[RowChange::Update { key: vec![("id".into(), Value::Int(2))], set: vec![("label".into(), Value::Text("y'z".into()))] }, RowChange::Delete { key: vec![("id".into(), Value::Int(1))] }],
        )
        .await
        .unwrap();
    assert_eq!(n, 2);
    assert!(d.apply_changes(None, "http_items", &[RowChange::Delete { key: vec![("id".into(), Value::Int(2))] }, RowChange::Insert { values: vec![("nope".into(), Value::Int(1))] }]).await.is_err());
    assert_eq!(d.count_rows(None, "http_items", None).await.unwrap(), 1);
    let page = d.fetch_page(&PageRequest { table: "http_items".into(), limit: 10, ..Default::default() }).await.unwrap();
    assert_eq!(page.rows[0][2], Value::Text("y'z".into()));

    // SQL errors come back as query errors.
    let e = d.execute("SELECT * FROM no_such_table").await.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(!e.is_empty() && !e.starts_with("connessione"), "{e}");
    exec(&d, "DROP TABLE http_items; DROP TABLE http_groups").await;
}

#[tokio::test]
async fn postgres_through_http_tunnel() {
    if !enabled() {
        return;
    }
    let d = db_drivers::connect(&cfg(DbKind::Postgres, 55432, "dbcraft", tunnel(false)), None).await.unwrap();
    roundtrip(d).await;
}

#[tokio::test]
async fn mysql_through_http_tunnel_base64() {
    if !enabled() {
        return;
    }
    let d = db_drivers::connect(&cfg(DbKind::MySql, 53306, "dbcraft", tunnel(true)), None).await.unwrap();
    roundtrip(d).await;
}

#[tokio::test]
async fn tunnel_security() {
    if !enabled() {
        return;
    }
    let err = |r: DbResult<Arc<dyn Driver>>| match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.to_string(),
    };
    let mut t = tunnel(false);
    t.token = "wrong".into();
    assert!(err(db_drivers::connect(&cfg(DbKind::Postgres, 55432, "dbcraft", t), None).await).contains("token"));
    let mut t = tunnel(false);
    t.basic_auth = false;
    assert!(err(db_drivers::connect(&cfg(DbKind::Postgres, 55432, "dbcraft", t), None).await).contains("401"));
    // Only host.docker.internal is allowed on this tunnel.
    let mut c = cfg(DbKind::Postgres, 55432, "dbcraft", tunnel(false));
    c.host = "10.1.2.3".into();
    assert!(err(db_drivers::connect(&c, None).await).contains("not allowed"));
    // SQL Server has no PDO driver in the tunnel.
    assert!(err(db_drivers::connect(&cfg(DbKind::SqlServer, 51433, "master", tunnel(false)), None).await).contains("non è supportato"));
}
