//! Integration tests against real servers started by `docker-compose.test.yml`.
//! Skipped unless `DBCRAFT_TEST_SERVERS=1`. Run with `--test-threads=1`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use db_core::*;

const PASSWORD: &str = "DbCraft_test1";

fn enabled() -> bool {
    std::env::var("DBCRAFT_TEST_SERVERS").is_ok_and(|v| v == "1")
}

fn cfg(kind: DbKind, port: u16, database: &str) -> ConnectionConfig {
    ConnectionConfig {
        id: format!("test-{kind:?}"),
        name: kind.label().into(),
        kind,
        host: "127.0.0.1".into(),
        port,
        user: kind.default_user().into(),
        password: PASSWORD.into(),
        database: database.into(),
        ..Default::default()
    }
}

/// Logical column types → the dialect's type names.
struct Types {
    int: &'static str,
    text: &'static str,
    decimal: &'static str,
    boolean: &'static str,
    date: &'static str,
    bytes: &'static str,
}

fn types(d: Dialect) -> Types {
    match d {
        Dialect::Postgres => Types { int: "integer", text: "varchar(100)", decimal: "numeric(10,2)", boolean: "boolean", date: "date", bytes: "bytea" },
        Dialect::MySql => Types { int: "int", text: "varchar(100)", decimal: "decimal(10,2)", boolean: "tinyint(1)", date: "date", bytes: "blob" },
        Dialect::SqlServer => Types { int: "int", text: "nvarchar(100)", decimal: "decimal(10,2)", boolean: "bit", date: "date", bytes: "varbinary(max)" },
        Dialect::Sqlite => Types { int: "INTEGER", text: "TEXT", decimal: "NUMERIC", boolean: "BOOLEAN", date: "DATE", bytes: "BLOB" },
    }
}

fn col(name: &str, ty: &str) -> ColumnDef {
    ColumnDef { name: name.into(), data_type: ty.into(), ..Default::default() }
}

fn text(v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        other => other.to_string(),
    }
}

async fn exec(drv: &Arc<dyn Driver>, sql: &str) {
    if let Err(e) = drv.execute(sql).await {
        panic!("{sql}\n→ {e}");
    }
}

async fn ignore(drv: &Arc<dyn Driver>, sql: &str) {
    let _ = drv.execute(sql).await;
}

async fn roundtrip(drv: Arc<dyn Driver>, schema: Option<&str>) {
    let d = drv.dialect();
    let t = types(d);
    let q = |n: &str| d.qualified(schema, n);

    // Clean slate.
    ignore(&drv, &format!("DROP VIEW {}", q("v_people"))).await;
    ignore(&drv, &format!("DROP TABLE {}", q("people"))).await;
    ignore(&drv, &format!("DROP TABLE {}", q("teams"))).await;

    let teams = TableDesign {
        schema: schema.map(Into::into),
        name: "teams".into(),
        columns: vec![ColumnDef { primary_key: true, not_null: true, ..col("id", t.int) }, ColumnDef { not_null: true, ..col("title", t.text) }],
        ..Default::default()
    };
    for s in d.create_table(&teams) {
        exec(&drv, &s).await;
    }
    let people = TableDesign {
        schema: schema.map(Into::into),
        name: "people".into(),
        columns: vec![
            ColumnDef { primary_key: true, not_null: true, auto_increment: true, ..col("id", t.int) },
            ColumnDef { not_null: true, comment: if d == Dialect::Postgres || d == Dialect::MySql { "full name".into() } else { String::new() }, ..col("name", t.text) },
            ColumnDef { default: "0".into(), ..col("score", t.decimal) },
            col("active", t.boolean),
            col("born", t.date),
            col("data", t.bytes),
            col("team_id", t.int),
        ],
        indexes: vec![IndexDef { name: "ix_people_name".into(), columns: "name".into(), unique: false }],
        foreign_keys: vec![ForeignKeyDef {
            name: "fk_people_team".into(),
            columns: "team_id".into(),
            ref_table: "teams".into(),
            ref_columns: "id".into(),
            on_delete: "CASCADE".into(),
            on_update: String::new(),
        }],
    };
    for s in d.create_table(&people) {
        exec(&drv, &s).await;
    }

    // Objects and views.
    exec(&drv, &format!("CREATE VIEW {} AS SELECT id, name FROM {}", q("v_people"), q("people"))).await;
    let objs = drv.list_objects(schema).await.unwrap();
    assert!(objs.iter().any(|o| o.name == "people" && o.kind == ObjectKind::Table), "{objs:?}");
    assert!(objs.iter().any(|o| o.name == "v_people" && o.kind == ObjectKind::View), "{objs:?}");

    // Multi-statement script: an insert, then a select.
    let (yes, bytes) = match d {
        Dialect::Postgres => ("TRUE", "'\\x0102ff'::bytea"),
        Dialect::SqlServer => ("1", "0x0102FF"),
        _ => ("1", "X'0102FF'"),
    };
    let nstr = |s: &str| d.string_literal(s);
    let script = format!(
        "INSERT INTO {teams} (id, title) VALUES (1, {a}), (2, {b});\n\
         INSERT INTO {people} (name, score, active, born, data, team_id) VALUES ({ada}, 1.5, {yes}, '1990-01-02', {bytes}, 1), ({bob}, NULL, NULL, NULL, NULL, 2);\n\
         SELECT id, name, score, active, born, data, team_id FROM {people} ORDER BY id",
        teams = q("teams"),
        people = q("people"),
        a = nstr("Rossi"),
        b = nstr("Blu"),
        ada = nstr("Adà l'ultima"),
        bob = nstr("Bob"),
    );
    let rs = drv.execute(&script).await.unwrap();
    let sel = rs.iter().find(|r| !r.columns.is_empty()).expect("a result with columns");
    assert_eq!(sel.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["id", "name", "score", "active", "born", "data", "team_id"]);
    assert_eq!(sel.rows.len(), 2);
    let r0 = &sel.rows[0];
    assert_eq!(r0[0], Value::Int(1), "id");
    assert_eq!(r0[1], Value::Text("Adà l'ultima".into()), "unicode text");
    assert_eq!(text(&r0[2]), "1.50", "decimal");
    assert!(matches!(r0[3], Value::Bool(true) | Value::Int(1)), "bool: {:?}", r0[3]);
    assert!(text(&r0[4]).starts_with("1990-01-02"), "date: {:?}", r0[4]);
    assert_eq!(r0[5], Value::Bytes(vec![1, 2, 255]), "bytes");
    assert_eq!(r0[6], Value::Int(1));
    assert!(sel.rows[1][2].is_null() && sel.rows[1][3].is_null() && sel.rows[1][5].is_null());

    // Introspection.
    let info = drv.describe_table(schema, "people").await.unwrap();
    assert_eq!(info.primary_key(), vec!["id".to_string()]);
    let c = |n: &str| info.columns.iter().find(|c| c.name == n).unwrap();
    assert!(c("id").auto_increment, "auto increment");
    assert!(!c("name").nullable && c("score").nullable);
    assert!(c("score").default.as_deref().is_some_and(|x| x.contains('0')), "default: {:?}", c("score").default);
    if d == Dialect::Postgres || d == Dialect::MySql {
        assert_eq!(c("name").comment.as_deref(), Some("full name"));
    }
    assert!(info.indexes.iter().any(|i| i.name == "ix_people_name" && i.columns == ["name"]), "{:?}", info.indexes);
    let fk = info.foreign_keys.iter().find(|f| f.name == "fk_people_team").expect("fk");
    assert_eq!((fk.columns.clone(), fk.ref_table.as_str(), fk.ref_columns.clone()), (vec!["team_id".to_string()], "teams", vec!["id".to_string()]));
    assert_eq!(fk.on_delete.as_deref(), Some("CASCADE"));

    // Pending edits applied in one transaction.
    let n = drv
        .apply_changes(
            schema,
            "people",
            &[
                RowChange::Update { key: vec![("id".into(), Value::Int(2))], set: vec![("score".into(), Value::Text("9.25".into())), ("name".into(), Value::Text("Bobby".into()))] },
                RowChange::Insert { values: vec![("name".into(), Value::Text("Cy".into())), ("team_id".into(), Value::Text("1".into()))] },
                RowChange::Delete { key: vec![("id".into(), Value::Int(1))] },
            ],
        )
        .await
        .unwrap();
    assert_eq!(n, 3);
    assert_eq!(drv.count_rows(schema, "people", None).await.unwrap(), 2);
    let page = drv
        .fetch_page(&PageRequest { schema: schema.map(Into::into), table: "people".into(), order: Some(("id".into(), true)), limit: 1, ..Default::default() })
        .await
        .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0][1], Value::Text("Cy".into()));
    let filtered = drv.count_rows(schema, "people", Some(&format!("{} = {}", d.quote("name"), nstr("Bobby")))).await.unwrap();
    assert_eq!(filtered, 1);

    // A failing change set rolls back entirely.
    let bad = drv
        .apply_changes(
            schema,
            "people",
            &[RowChange::Delete { key: vec![("id".into(), Value::Int(2))] }, RowChange::Insert { values: vec![("no_such_column".into(), Value::Int(1))] }],
        )
        .await;
    assert!(bad.is_err());
    assert_eq!(drv.count_rows(schema, "people", None).await.unwrap(), 2);

    // Designer: rename, add, change type, drop an index; keep the data.
    exec(&drv, &format!("DROP VIEW {}", q("v_people"))).await;
    let old = TableDesign::from_info(&info);
    let mut new = old.clone();
    let pos = new.columns.iter().position(|c| c.name == "name").unwrap();
    new.columns[pos].name = "full_name".into();
    new.indexes.clear();
    new.columns.push(ColumnDef { comment: String::new(), ..col("note", t.text) });
    let sc = new.columns.iter().position(|c| c.name == "score").unwrap();
    new.columns[sc].data_type = match d {
        Dialect::Postgres => "numeric(12,3)".into(),
        _ => "decimal(12,3)".into(),
    };
    for s in d.alter_table(&old, &new) {
        if !s.starts_with("--") {
            exec(&drv, &s).await;
        }
    }
    let info2 = drv.describe_table(schema, "people").await.unwrap();
    let names: Vec<&str> = info2.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"full_name") && names.contains(&"note") && !names.contains(&"name"), "{names:?}");
    assert!(info2.columns.iter().find(|c| c.name == "score").unwrap().data_type.contains("12,3"), "{:?}", info2.columns);
    assert!(info2.indexes.is_empty(), "{:?}", info2.indexes);
    assert_eq!(drv.count_rows(schema, "people", None).await.unwrap(), 2);
    // Nothing to do when the design didn't change.
    assert!(d.alter_table(&TableDesign::from_info(&info2), &TableDesign::from_info(&info2)).is_empty());

    exec(&drv, &format!("DROP TABLE {}", q("people"))).await;
    exec(&drv, &format!("DROP TABLE {}", q("teams"))).await;
    drv.close().await;
}

#[tokio::test]
async fn postgres() {
    if !enabled() {
        return;
    }
    let drv = db_drivers::connect(&cfg(DbKind::Postgres, 55432, "dbcraft"), None).await.unwrap();
    assert!(drv.list_databases().await.unwrap().contains(&"dbcraft".to_string()));
    assert!(drv.list_schemas().await.unwrap().contains(&"public".to_string()));
    ignore(&drv, "CREATE SCHEMA app").await;
    roundtrip(drv.clone(), Some("app")).await;
}

#[tokio::test]
async fn mysql() {
    if !enabled() {
        return;
    }
    let drv = db_drivers::connect(&cfg(DbKind::MySql, 53306, ""), Some("dbcraft")).await.unwrap();
    assert!(drv.list_databases().await.unwrap().contains(&"dbcraft".to_string()));
    assert!(drv.list_schemas().await.unwrap().is_empty());
    roundtrip(drv, None).await;
}

#[tokio::test]
async fn mariadb() {
    if !enabled() {
        return;
    }
    let drv = db_drivers::connect(&cfg(DbKind::MariaDb, 53307, "dbcraft"), None).await.unwrap();
    roundtrip(drv, None).await;
}

#[tokio::test]
async fn sqlserver() {
    if !enabled() {
        return;
    }
    let master = db_drivers::connect(&cfg(DbKind::SqlServer, 51433, "master"), None).await.unwrap();
    ignore(&master, "CREATE DATABASE dbcraft").await;
    assert!(master.list_databases().await.unwrap().contains(&"dbcraft".to_string()));
    let drv = db_drivers::connect(&cfg(DbKind::SqlServer, 51433, "master"), Some("dbcraft")).await.unwrap();
    assert!(drv.list_schemas().await.unwrap().contains(&"dbo".to_string()));
    roundtrip(drv, Some("dbo")).await;
}

/// Every value of `sql`'s single row must decode to something non-NULL.
async fn types_smoke(drv: Arc<dyn Driver>, sql: &str) {
    let rs = drv.execute(sql).await.unwrap();
    let r = rs.iter().find(|r| !r.columns.is_empty()).expect("rows");
    for (c, v) in r.columns.iter().zip(&r.rows[0]) {
        println!("{:<14} {:<18} {:?}", c.name, c.type_name, v);
    }
    let nulls: Vec<&str> = r.columns.iter().zip(&r.rows[0]).filter(|(_, v)| v.is_null()).map(|(c, _)| c.name.as_str()).collect();
    assert!(nulls.is_empty(), "decoded as NULL: {nulls:?}");
}

#[tokio::test]
async fn postgres_types() {
    if !enabled() {
        return;
    }
    let drv = db_drivers::connect(&cfg(DbKind::Postgres, 55432, "dbcraft"), None).await.unwrap();
    types_smoke(
        drv,
        "SELECT now()::timestamp AS ts, '2020-01-01 10:00+02'::timestamptz AS tstz, gen_random_uuid() AS uuid, '{\"a\":1}'::jsonb AS jsonb, \
         '[1]'::json AS json, ARRAY[1,2] AS arr, interval '1 day 2 hours' AS iv, 12.5::money AS money, 'x'::char(3) AS ch, 1::smallint AS i2, \
         2::bigint AS i8, 1.5::real AS f4, 2.5::float8 AS f8, inet '10.0.0.1' AS inet, '10:00'::time AS t, 'abc'::bytea AS by, true AS b, \
         12345678901234567890.123::numeric AS big, 'a'::name AS nm, int4range(1, 5) AS rng, point(1, 2) AS pt",
    )
    .await;
}

#[tokio::test]
async fn mysql_types() {
    if !enabled() {
        return;
    }
    let drv = db_drivers::connect(&cfg(DbKind::MySql, 53306, "dbcraft"), None).await.unwrap();
    types_smoke(
        drv,
        "SELECT NOW() AS dt, CURDATE() AS d, CAST(1.5 AS DECIMAL(5,2)) AS dec5, JSON_OBJECT('a', 1) AS js, 18446744073709551615 AS u64, \
         CAST(1 AS UNSIGNED) AS u, b'101' AS bits, UUID() AS uuid, TIME('10:00:00') AS t, YEAR(NOW()) AS y, 1.5e0 AS dbl, _binary 'ab' AS bin, \
         CAST('x' AS CHAR) AS ch, CAST(-5 AS SIGNED) AS neg, TRUE AS bool, ST_GeomFromText('POINT(1 1)') AS geo",
    )
    .await;
}

#[tokio::test]
async fn sqlserver_types() {
    if !enabled() {
        return;
    }
    let drv = db_drivers::connect(&cfg(DbKind::SqlServer, 51433, "master"), None).await.unwrap();
    types_smoke(
        drv,
        "SELECT SYSDATETIME() AS dt2, GETDATE() AS dt, SYSDATETIMEOFFSET() AS dto, NEWID() AS guid, CAST(1.5 AS money) AS m, \
         CAST(1 AS bit) AS b, CAST(1 AS tinyint) AS ti, CAST('<a/>' AS xml) AS x, CAST(0x01 AS varbinary(4)) AS vb, CAST('a' AS char(3)) AS ch, \
         CAST(1.25 AS float) AS f, CAST('10:00' AS time) AS t, CAST('2020-01-01' AS date) AS d, CAST('2020-01-01' AS smalldatetime) AS sdt, \
         CAST(1.5 AS decimal(5,2)) AS dec5, CAST(N'è' AS nvarchar(10)) AS nv, CAST(1 AS bigint) AS bi, CAST(2 AS smallint) AS si, \
         CAST(1.5 AS real) AS r, CAST(2.5 AS smallmoney) AS sm",
    )
    .await;
}
