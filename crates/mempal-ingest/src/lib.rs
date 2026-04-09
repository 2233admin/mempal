#![warn(clippy::all)]

pub mod chunk;
pub mod detect;
pub mod normalize;

use std::path::{Path, PathBuf};

use mempal_core::{
    db::Database,
    types::{Drawer, SourceType},
    utils::{build_drawer_id, current_timestamp, route_room_from_taxonomy},
};
use mempal_embed::{EmbedError, Embedder};
use thiserror::Error;

use crate::{
    chunk::{chunk_conversation, chunk_text},
    detect::{Format, detect_format},
    normalize::{NormalizeError, normalize_content},
};

const CHUNK_WINDOW: usize = 800;
const CHUNK_OVERLAP: usize = 100;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestStats {
    pub files: usize,
    pub chunks: usize,
    pub skipped: usize,
}

pub type Result<T> = std::result::Result<T, IngestError>;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("failed to read {path}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to normalize {path}")]
    Normalize {
        path: PathBuf,
        #[source]
        source: NormalizeError,
    },
    #[error("failed to load taxonomy for wing {wing}")]
    LoadTaxonomy {
        wing: String,
        #[source]
        source: mempal_core::db::DbError,
    },
    #[error("failed to embed chunks from {path}")]
    EmbedChunks {
        path: PathBuf,
        #[source]
        source: EmbedError,
    },
    #[error("failed to check drawer {drawer_id}")]
    CheckDrawer {
        drawer_id: String,
        #[source]
        source: mempal_core::db::DbError,
    },
    #[error("failed to insert drawer {drawer_id}")]
    InsertDrawer {
        drawer_id: String,
        #[source]
        source: mempal_core::db::DbError,
    },
    #[error("failed to insert vector for {drawer_id}")]
    InsertVector {
        drawer_id: String,
        #[source]
        source: mempal_core::db::DbError,
    },
    #[error("failed to read directory {path}")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read entry in {path}")]
    ReadDirEntry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub async fn ingest_file<E: Embedder + ?Sized>(
    db: &Database,
    embedder: &E,
    path: &Path,
    wing: &str,
    room: Option<&str>,
) -> Result<IngestStats> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| IngestError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
    let content = String::from_utf8_lossy(&bytes).to_string();
    if content.trim().is_empty() {
        return Ok(IngestStats {
            files: 1,
            ..IngestStats::default()
        });
    }

    let format = detect_format(&content);
    let normalized = normalize_content(&content, format).map_err(|source| IngestError::Normalize {
        path: path.to_path_buf(),
        source,
    })?;
    let resolved_room = match room {
        Some(room) => room.to_string(),
        None => {
            let taxonomy = db.taxonomy_entries().map_err(|source| IngestError::LoadTaxonomy {
                wing: wing.to_string(),
                source,
            })?;
            route_room_from_taxonomy(&normalized, wing, &taxonomy)
        }
    };
    let chunks = match format {
        Format::ClaudeJsonl | Format::ChatGptJson => chunk_conversation(&normalized),
        Format::PlainText => chunk_text(&normalized, CHUNK_WINDOW, CHUNK_OVERLAP),
    };
    if chunks.is_empty() {
        return Ok(IngestStats {
            files: 1,
            ..IngestStats::default()
        });
    }

    let chunk_refs = chunks.iter().map(String::as_str).collect::<Vec<_>>();
    let vectors = embedder
        .embed(&chunk_refs)
        .await
        .map_err(|source| IngestError::EmbedChunks {
            path: path.to_path_buf(),
            source,
        })?;

    let mut stats = IngestStats {
        files: 1,
        ..IngestStats::default()
    };

    for (chunk_index, (chunk, vector)) in chunks.iter().zip(vectors.iter()).enumerate() {
        let drawer_id = build_drawer_id(wing, Some(resolved_room.as_str()), chunk);
        if db
            .drawer_exists(&drawer_id)
            .map_err(|source| IngestError::CheckDrawer {
                drawer_id: drawer_id.clone(),
                source,
            })?
        {
            stats.skipped += 1;
            continue;
        }

        let drawer = Drawer {
            id: drawer_id.clone(),
            content: chunk.clone(),
            wing: wing.to_string(),
            room: Some(resolved_room.clone()),
            source_file: Some(path.to_string_lossy().to_string()),
            source_type: source_type_for(format),
            added_at: current_timestamp(),
            chunk_index: Some(chunk_index as i64),
        };

        db.insert_drawer(&drawer)
            .map_err(|source| IngestError::InsertDrawer {
                drawer_id: drawer.id.clone(),
                source,
            })?;
        db.insert_vector(&drawer_id, vector)
            .map_err(|source| IngestError::InsertVector {
                drawer_id: drawer.id.clone(),
                source,
            })?;
        stats.chunks += 1;
    }

    Ok(stats)
}

pub async fn ingest_dir<E: Embedder + ?Sized>(
    db: &Database,
    embedder: &E,
    dir: &Path,
    wing: &str,
    room: Option<&str>,
) -> Result<IngestStats> {
    let mut stats = IngestStats::default();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).map_err(|source| IngestError::ReadDir {
            path: current.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| IngestError::ReadDirEntry {
                path: current.clone(),
                source,
            })?;
            let path = entry.path();

            if path.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                stack.push(path);
                continue;
            }

            if path.is_file() {
                let file_stats = ingest_file(db, embedder, &path, wing, room).await?;
                stats.files += file_stats.files;
                stats.chunks += file_stats.chunks;
                stats.skipped += file_stats.skipped;
            }
        }
    }

    Ok(stats)
}

fn source_type_for(format: Format) -> SourceType {
    match format {
        Format::ClaudeJsonl | Format::ChatGptJson => SourceType::Conversation,
        Format::PlainText => SourceType::Project,
    }
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| matches!(name, ".git" | "target" | "node_modules"))
        .unwrap_or(false)
}
