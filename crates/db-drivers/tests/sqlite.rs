#![allow(clippy::unwrap_used)]

use db_core::*;

#[tokio::test]
async fn sqlite_roundtrip() {
    let cfg = ConnectionConfig { kind: DbKind::Sqlite, database: ":memory:".into(), ..Default::default() };
    let drv = db_drivers::connect(&cfg, None).await.unwrap();
    let d = drv.dialect();

    let design = TableDesign {
        name: "people".into(),
        columns: vec![
            ColumnDef { name: "id".into(), data_type: "INTEGER".into(), primary_key: true, auto_increment: true, ..Default::default() },
            ColumnDef { name: "name".into(), data_type: "TEXT".into(), not_null: true, ..Default::default() },
            ColumnDef { name: "score".into(), data_type: "REAL".into(), ..Default::default() },
        ],
        indexes: vec![IndexDef { name: "ix_people_name".into(), columns: "name".into(), unique: false }],
        ..Default::default()
    };
    for s in d.create_table(&design) {
        drv.execute(&s).await.unwrap();
    }
    let rs = drv.execute("INSERT INTO people (name, score) VALUES ('Ada', 1.5), ('Bob', NULL); SELECT * FROM people ORDER BY id").await.unwrap();
    assert_eq!(rs.len(), 2);
    assert_eq!(rs[0].affected, Some(2));
    assert_eq!(rs[1].rows.len(), 2);
    assert_eq!(rs[1].rows[0], vec![Value::Int(1), Value::Text("Ada".into()), Value::Float(1.5)]);
    assert_eq!(rs[1].rows[1][2], Value::Null);

    let objs = drv.list_objects(None).await.unwrap();
    assert!(objs.iter().any(|o| o.name == "people" && o.kind == ObjectKind::Table));

    let info = drv.describe_table(None, "people").await.unwrap();
    assert_eq!(info.primary_key(), vec!["id".to_string()]);
    assert!(info.columns[0].auto_increment);
    assert!(!info.columns[1].nullable);
    assert_eq!(info.indexes.len(), 1);

    let n = drv
        .apply_changes(
            None,
            "people",
            &[
                RowChange::Update { key: vec![("id".into(), Value::Int(2))], set: vec![("score".into(), Value::Text("9".into()))] },
                RowChange::Insert { values: vec![("name".into(), Value::Text("Cy".into()))] },
                RowChange::Delete { key: vec![("id".into(), Value::Int(1))] },
            ],
        )
        .await
        .unwrap();
    assert_eq!(n, 3);
    assert_eq!(drv.count_rows(None, "people", None).await.unwrap(), 2);
    let page = drv.fetch_page(&PageRequest { table: "people".into(), order: vec![("id".into(), true)], limit: 1, ..Default::default() }).await.unwrap();
    assert_eq!(page.rows[0][1], Value::Text("Cy".into()));

    // Designer: change a type → SQLite rebuild keeps the data.
    let old = TableDesign::from_info(&info);
    let mut new = old.clone();
    new.columns[2].data_type = "TEXT".into();
    new.columns.push(ColumnDef { name: "note".into(), data_type: "TEXT".into(), ..Default::default() });
    for s in d.alter_table(&old, &new) {
        drv.execute(&s).await.unwrap();
    }
    let info2 = drv.describe_table(None, "people").await.unwrap();
    assert_eq!(info2.columns.len(), 4);
    assert_eq!(info2.columns[2].data_type, "TEXT");
    assert_eq!(drv.count_rows(None, "people", None).await.unwrap(), 2);
    assert_eq!(info2.indexes.len(), 1);

    // A failing change set rolls back.
    let bad = drv.apply_changes(None, "people", &[RowChange::Delete { key: vec![("id".into(), Value::Int(2))] }, RowChange::Insert { values: vec![("nope".into(), Value::Int(1))] }]).await;
    assert!(bad.is_err());
    assert_eq!(drv.count_rows(None, "people", None).await.unwrap(), 2);

    // A change that would touch more than one row (a key that is not unique) saves nothing.
    drv.execute("UPDATE people SET score = 5").await.unwrap();
    let wide = drv
        .apply_changes(
            None,
            "people",
            &[
                RowChange::Update { key: vec![("id".into(), Value::Int(2))], set: vec![("name".into(), Value::Text("first".into()))] },
                RowChange::Update { key: vec![("score".into(), Value::Int(5))], set: vec![("name".into(), Value::Text("all".into()))] },
            ],
        )
        .await;
    assert!(wide.is_err_and(|e| e.to_string().contains("invece di una")));
    let names = drv.execute("SELECT name FROM people WHERE name IN ('first', 'all')").await.unwrap();
    assert!(names[0].rows.is_empty());
}
