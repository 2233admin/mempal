use mempal_core::db::Database;
use mempal_core::types::{Drawer, SourceType};
use rusqlite::Connection;
use rusqlite::Row;
use tempfile::tempdir;

#[test]
fn test_db_init() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("test.db");
    let db = Database::open(&path).expect("database should open");

    assert!(path.exists());

    let tables: Vec<String> = db
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .expect("table query should prepare")
        .query_map([], |row: &Row<'_>| row.get::<_, String>(0))
        .expect("table query should run")
        .collect::<Result<Vec<_>, _>>()
        .expect("table rows should collect");

    assert!(tables.contains(&"drawers".to_string()));
    assert!(tables.contains(&"drawer_vectors".to_string()));
    assert!(tables.contains(&"triples".to_string()));
    assert!(tables.contains(&"taxonomy".to_string()));

    let schema_version: u32 = db.schema_version().expect("schema version should load");
    assert_eq!(schema_version, 1);

    let indexes: Vec<String> = db
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")
        .expect("index query should prepare")
        .query_map([], |row: &Row<'_>| row.get::<_, String>(0))
        .expect("index query should run")
        .collect::<Result<Vec<_>, _>>()
        .expect("index rows should collect");

    assert!(indexes.contains(&"idx_drawers_wing".to_string()));
    assert!(indexes.contains(&"idx_drawers_wing_room".to_string()));
}

#[test]
fn test_db_idempotent() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("test.db");
    let db = Database::open(&path).expect("database should open");

    db.insert_drawer(&Drawer {
        id: "test1".into(),
        content: "hello".into(),
        wing: "w".into(),
        room: None,
        source_file: None,
        source_type: SourceType::Manual,
        added_at: "2026-04-08".into(),
        chunk_index: None,
    })
    .expect("drawer insert should succeed");

    drop(db);

    let reopened = Database::open(&path).expect("database should reopen");
    let count: i64 = reopened
        .conn()
        .query_row("SELECT COUNT(*) FROM drawers", [], |row: &Row<'_>| {
            row.get::<_, i64>(0)
        })
        .expect("count query should succeed");

    assert_eq!(count, 1);
    assert_eq!(
        reopened
            .schema_version()
            .expect("schema version should load after reopen"),
        1
    );
}

#[test]
fn test_db_migrates_legacy_schema_without_user_version() {
    let dir = tempdir().expect("temp dir should be created");
    let path = dir.path().join("legacy.db");
    let conn = Connection::open(&path).expect("legacy db should open");
    conn.execute_batch(
        r#"
        CREATE TABLE drawers (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            wing TEXT NOT NULL,
            room TEXT,
            source_file TEXT,
            source_type TEXT NOT NULL,
            added_at TEXT NOT NULL,
            chunk_index INTEGER
        );
        INSERT INTO drawers (id, content, wing, room, source_file, source_type, added_at, chunk_index)
        VALUES ('legacy', 'hello', 'myapp', NULL, 'README.md', 'project', '2026-04-10', 0);
        "#,
    )
    .expect("legacy schema should initialize");
    drop(conn);

    let db = Database::open(&path).expect("database should migrate legacy schema");

    assert_eq!(
        db.schema_version()
            .expect("schema version should be upgraded"),
        1
    );

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM drawers", [], |row: &Row<'_>| {
            row.get::<_, i64>(0)
        })
        .expect("drawer count query should succeed");
    assert_eq!(count, 1);
}
