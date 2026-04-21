//! Integration tests for P12 stage-1 mind-model bootstrap schema/core work.

use mempal::core::db::Database;
use mempal::core::types::{AnchorKind, Drawer, MemoryDomain, MemoryKind, Provenance, SourceType};
use rusqlite::Connection;
use tempfile::TempDir;

fn create_v4_db(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open v4 db");
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE drawers (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            wing TEXT NOT NULL,
            room TEXT,
            source_file TEXT,
            source_type TEXT NOT NULL CHECK(source_type IN ('project', 'conversation', 'manual')),
            added_at TEXT NOT NULL,
            chunk_index INTEGER,
            deleted_at TEXT,
            importance INTEGER DEFAULT 0
        );

        CREATE TABLE triples (
            id TEXT PRIMARY KEY,
            subject TEXT NOT NULL,
            predicate TEXT NOT NULL,
            object TEXT NOT NULL,
            valid_from TEXT,
            valid_to TEXT,
            confidence REAL DEFAULT 1.0,
            source_drawer TEXT REFERENCES drawers(id)
        );

        CREATE TABLE taxonomy (
            wing TEXT NOT NULL,
            room TEXT NOT NULL DEFAULT '',
            display_name TEXT,
            keywords TEXT,
            PRIMARY KEY (wing, room)
        );

        CREATE INDEX idx_drawers_wing ON drawers(wing);
        CREATE INDEX idx_drawers_wing_room ON drawers(wing, room);
        CREATE INDEX idx_drawers_deleted_at ON drawers(deleted_at);
        CREATE INDEX idx_triples_subject ON triples(subject);
        CREATE INDEX idx_triples_object ON triples(object);

        CREATE VIRTUAL TABLE drawers_fts USING fts5(
            content,
            content='drawers',
            content_rowid='rowid'
        );

        CREATE TRIGGER drawers_ai AFTER INSERT ON drawers BEGIN
            INSERT INTO drawers_fts(rowid, content) VALUES (new.rowid, new.content);
        END;

        CREATE TRIGGER drawers_au_softdelete AFTER UPDATE OF deleted_at ON drawers
            WHEN new.deleted_at IS NOT NULL AND old.deleted_at IS NULL BEGIN
            INSERT INTO drawers_fts(drawers_fts, rowid, content)
            VALUES ('delete', old.rowid, old.content);
        END;

        PRAGMA user_version = 4;
        "#,
    )
    .expect("apply v4 schema");
}

#[test]
fn test_migration_backfills_legacy_drawers_with_bootstrap_defaults() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("palace.db");
    create_v4_db(&db_path);

    {
        let conn = Connection::open(&db_path).expect("reopen v4 db");
        conn.execute(
            r#"
            INSERT INTO drawers (
                id,
                content,
                wing,
                room,
                source_file,
                source_type,
                added_at,
                chunk_index,
                importance
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            (
                "drawer_legacy_001",
                "Legacy evidence body",
                "mempal",
                Some("bootstrap"),
                Some("docs/specs/legacy.md"),
                "project",
                "1710000000",
                Some(0_i64),
                4_i32,
            ),
        )
        .expect("insert legacy drawer");
    }

    let db = Database::open(&db_path).expect("migrate db to latest");
    assert_eq!(db.schema_version().expect("schema version"), 5);

    let drawer = db
        .get_drawer("drawer_legacy_001")
        .expect("load drawer")
        .expect("drawer exists");

    assert_eq!(drawer.memory_kind, MemoryKind::Evidence);
    assert_eq!(drawer.domain, MemoryDomain::Project);
    assert_eq!(drawer.field, "general");
    assert_eq!(drawer.anchor_kind, AnchorKind::Repo);
    assert_eq!(drawer.anchor_id, "repo://legacy");
    assert_eq!(drawer.parent_anchor_id, None);
    assert_eq!(drawer.provenance, Some(Provenance::Research));
    assert_eq!(drawer.statement, None);
    assert_eq!(drawer.tier, None);
    assert_eq!(drawer.status, None);
    assert!(drawer.supporting_refs.is_empty());
    assert!(drawer.counterexample_refs.is_empty());
    assert!(drawer.teaching_refs.is_empty());
    assert!(drawer.verification_refs.is_empty());
    assert_eq!(drawer.scope_constraints, None);
    assert_eq!(drawer.trigger_hints, None);
}

#[test]
fn test_global_anchor_rejected_for_non_global_domain() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("palace.db");
    let db = Database::open(&db_path).expect("open db");

    let drawer = Drawer {
        id: "drawer_invalid_anchor".to_string(),
        content: "repo-local note".to_string(),
        wing: "mempal".to_string(),
        room: Some("bootstrap".to_string()),
        source_file: Some("tests://mind-model".to_string()),
        source_type: SourceType::Manual,
        added_at: "1710001234".to_string(),
        chunk_index: None,
        importance: 0,
        memory_kind: MemoryKind::Evidence,
        domain: MemoryDomain::Project,
        field: "general".to_string(),
        anchor_kind: AnchorKind::Global,
        anchor_id: "global://all".to_string(),
        parent_anchor_id: None,
        provenance: Some(Provenance::Human),
        statement: None,
        tier: None,
        status: None,
        supporting_refs: Vec::new(),
        counterexample_refs: Vec::new(),
        teaching_refs: Vec::new(),
        verification_refs: Vec::new(),
        scope_constraints: None,
        trigger_hints: None,
    };

    let error = db
        .insert_drawer(&drawer)
        .expect_err("global anchor should reject non-global domain");
    let message = error.to_string();
    assert!(
        message.contains("global") && message.contains("domain"),
        "unexpected error: {message}"
    );
}
