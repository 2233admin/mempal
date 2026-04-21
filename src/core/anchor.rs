use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::{AnchorKind, MemoryDomain, Provenance, SourceType};
use thiserror::Error;

pub const LEGACY_REPO_ANCHOR_ID: &str = "repo://legacy";
pub const DEFAULT_FIELD: &str = "general";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedAnchor {
    pub anchor_kind: AnchorKind,
    pub anchor_id: String,
    pub parent_anchor_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum AnchorError {
    #[error("cwd is required to derive anchor metadata")]
    MissingCwd,
    #[error("failed to canonicalize {path}: {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("git rev-parse {arg} failed for {cwd}: {stderr}")]
    Git {
        cwd: PathBuf,
        arg: &'static str,
        stderr: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapDefaults {
    pub field: String,
    pub anchor_kind: AnchorKind,
    pub anchor_id: String,
    pub parent_anchor_id: Option<String>,
    pub provenance: Provenance,
}

pub fn bootstrap_anchor() -> (AnchorKind, String, Option<String>) {
    (AnchorKind::Repo, LEGACY_REPO_ANCHOR_ID.to_string(), None)
}

pub fn bootstrap_provenance(source_type: &SourceType) -> Provenance {
    match source_type {
        SourceType::Project => Provenance::Research,
        SourceType::Conversation | SourceType::Manual => Provenance::Human,
    }
}

pub fn bootstrap_defaults(source_type: &SourceType) -> BootstrapDefaults {
    let (anchor_kind, anchor_id, parent_anchor_id) = bootstrap_anchor();
    BootstrapDefaults {
        field: DEFAULT_FIELD.to_string(),
        anchor_kind,
        anchor_id,
        parent_anchor_id,
        provenance: bootstrap_provenance(source_type),
    }
}

pub fn derive_anchor_from_cwd(cwd: Option<&Path>) -> Result<DerivedAnchor, AnchorError> {
    let cwd = cwd.ok_or(AnchorError::MissingCwd)?;
    let canonical_cwd = cwd
        .canonicalize()
        .map_err(|source| AnchorError::Canonicalize {
            path: cwd.to_path_buf(),
            source,
        })?;

    let worktree_root = match git_rev_parse(&canonical_cwd, "--show-toplevel") {
        Ok(root) => canonicalize_path(Path::new(root.trim()))?,
        Err(AnchorError::Git { .. }) => {
            return Ok(DerivedAnchor {
                anchor_kind: AnchorKind::Worktree,
                anchor_id: worktree_anchor_id(&canonical_cwd),
                parent_anchor_id: None,
            });
        }
        Err(error) => return Err(error),
    };

    let common_dir_raw = git_rev_parse(&worktree_root, "--git-common-dir")?;
    let common_dir_path = resolve_git_path(&worktree_root, common_dir_raw.trim());
    let common_dir = canonicalize_path(&common_dir_path)?;

    Ok(DerivedAnchor {
        anchor_kind: AnchorKind::Worktree,
        anchor_id: worktree_anchor_id(&worktree_root),
        parent_anchor_id: Some(repo_anchor_id(&common_dir)),
    })
}

pub fn validate_anchor_domain(
    domain: &MemoryDomain,
    anchor_kind: &AnchorKind,
) -> Result<(), &'static str> {
    if matches!(anchor_kind, AnchorKind::Global) && !matches!(domain, MemoryDomain::Global) {
        return Err("global anchor requires domain=global");
    }
    Ok(())
}

fn worktree_anchor_id(path: &Path) -> String {
    format!("worktree://{}", path.display())
}

fn repo_anchor_id(path: &Path) -> String {
    format!("repo://{}", path.display())
}

fn canonicalize_path(path: &Path) -> Result<PathBuf, AnchorError> {
    path.canonicalize()
        .map_err(|source| AnchorError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}

fn resolve_git_path(base: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn git_rev_parse(cwd: &Path, arg: &'static str) -> Result<String, AnchorError> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg(arg)
        .current_dir(cwd)
        .output()
        .map_err(|source| AnchorError::Canonicalize {
            path: cwd.to_path_buf(),
            source,
        })?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    Err(AnchorError::Git {
        cwd: cwd.to_path_buf(),
        arg,
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}
