use std::fs;
use std::path::Path;

use mempal_core::db::Database;
use mempal_embed::Embedder;
use mempal_ingest::{ingest_dir, ingest_file};
use tempfile::tempdir;

#[derive(Default)]
struct TestEmbedder;

#[async_trait::async_trait]
impl Embedder for TestEmbedder {
    async fn embed(
        &self,
        texts: &[&str],
    ) -> std::result::Result<Vec<Vec<f32>>, mempal_embed::EmbedError> {
        Ok(texts.iter().map(|text| fake_embedding(text)).collect())
    }

    fn dimensions(&self) -> usize {
        384
    }

    fn name(&self) -> &str {
        "test"
    }
}

fn fake_embedding(text: &str) -> Vec<f32> {
    let mut embedding = vec![0.0_f32; 384];
    for (index, byte) in text.bytes().enumerate() {
        embedding[index % 384] += f32::from(byte) / 255.0;
    }
    embedding
}

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect("test fixture should be written");
}

fn insert_taxonomy(db: &Database, wing: &str, room: &str, keywords: &[&str]) {
    let keywords = serde_json::to_string(keywords).expect("keywords should serialize");
    db.conn()
        .execute(
            "INSERT INTO taxonomy (wing, room, display_name, keywords) VALUES (?1, ?2, ?3, ?4)",
            (wing, room, room, keywords.as_str()),
        )
        .expect("taxonomy should insert");
}

#[tokio::test]
async fn test_ingest_text_file() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    let file = dir.path().join("readme.md");
    write_file(
        &file,
        "We decided to use PostgreSQL for the analytics database.",
    );

    let stats = ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("file ingest should succeed");
    assert!(stats.chunks > 0);

    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM drawers WHERE wing = 'myproject'",
            [],
            |row| row.get(0),
        )
        .expect("drawer count query should succeed");
    assert!(count > 0);

    let vector_count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM drawer_vectors", [], |row| row.get(0))
        .expect("vector count query should succeed");
    assert_eq!(vector_count, count);
}

#[tokio::test]
async fn test_ingest_dedup() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    let file = dir.path().join("notes.md");
    write_file(
        &file,
        "A stable ingest ID should deduplicate repeated imports.",
    );

    ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("first ingest should succeed");
    let second = ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("second ingest should succeed");

    assert_eq!(second.chunks, 0);

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))
        .expect("drawer count query should succeed");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_ingest_empty_file() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    let file = dir.path().join("empty.md");
    write_file(&file, "");

    let stats = ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("empty file ingest should not error");

    assert_eq!(stats.chunks, 0);

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))
        .expect("drawer count query should succeed");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_ingest_directory() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    let project_dir = dir.path().join("project");
    let src_dir = project_dir.join("src");
    let nested_dir = src_dir.join("nested");
    fs::create_dir_all(&nested_dir).expect("source directories should be created");
    fs::create_dir_all(project_dir.join(".git")).expect("ignored directory should be created");
    fs::create_dir_all(project_dir.join("target")).expect("ignored directory should be created");

    write_file(&src_dir.join("lib.rs"), "pub fn alpha() {}");
    write_file(&src_dir.join("main.rs"), "fn main() {}");
    write_file(&nested_dir.join("util.rs"), "pub fn beta() {}");
    write_file(&project_dir.join("README.md"), "Project notes live here.");
    write_file(&project_dir.join(".git").join("ignored.txt"), "ignore me");

    let stats = ingest_dir(&db, &embedder, &project_dir, "myproject", None)
        .await
        .expect("directory ingest should succeed");

    assert_eq!(stats.files, 4);

    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM drawers", [], |row| row.get(0))
        .expect("drawer count query should succeed");
    assert!(count >= 4);

    let mut statement = db
        .conn()
        .prepare("SELECT DISTINCT source_file FROM drawers ORDER BY source_file")
        .expect("source file query should prepare");
    let source_files = statement
        .query_map([], |row| row.get::<_, Option<String>>(0))
        .expect("source file query should run")
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("source files should load");

    assert_eq!(
        source_files,
        vec![
            Some("README.md".to_string()),
            Some("src/lib.rs".to_string()),
            Some("src/main.rs".to_string()),
            Some("src/nested/util.rs".to_string()),
        ]
    );
}

#[tokio::test]
async fn test_ingest_routes_room_from_taxonomy() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    insert_taxonomy(&db, "myproject", "auth", &["auth", "clerk", "login"]);

    let file = dir.path().join("decision.md");
    write_file(
        &file,
        "We switched login to Clerk because auth setup was simpler.",
    );

    ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("file ingest should succeed");

    let room: Option<String> = db
        .conn()
        .query_row("SELECT room FROM drawers LIMIT 1", [], |row| row.get(0))
        .expect("room query should succeed");
    assert_eq!(room.as_deref(), Some("auth"));
}

#[tokio::test]
async fn test_ingest_routes_to_default_room_when_no_taxonomy_match() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    insert_taxonomy(&db, "myproject", "auth", &["auth", "clerk", "login"]);

    let file = dir.path().join("notes.md");
    write_file(&file, "Deployment work moved to Fly.io last week.");

    ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("file ingest should succeed");

    let room: Option<String> = db
        .conn()
        .query_row("SELECT room FROM drawers LIMIT 1", [], |row| row.get(0))
        .expect("room query should succeed");
    assert_eq!(room.as_deref(), Some("default"));
}

#[tokio::test]
async fn test_ingest_file_stores_source_as_basename() {
    let dir = tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("database should open");
    let embedder = TestEmbedder;

    let nested_dir = dir.path().join("notes");
    fs::create_dir_all(&nested_dir).expect("nested dir should exist");
    let file = nested_dir.join("decision.md");
    write_file(&file, "Clerk replaced Auth0 for pricing reasons.");

    ingest_file(&db, &embedder, &file, "myproject", None)
        .await
        .expect("file ingest should succeed");

    let source_file: Option<String> = db
        .conn()
        .query_row("SELECT source_file FROM drawers LIMIT 1", [], |row| {
            row.get(0)
        })
        .expect("source file query should succeed");
    assert_eq!(source_file.as_deref(), Some("decision.md"));
}
