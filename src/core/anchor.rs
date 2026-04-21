use super::types::{AnchorKind, MemoryDomain, Provenance, SourceType};

pub const LEGACY_REPO_ANCHOR_ID: &str = "repo://legacy";
pub const DEFAULT_FIELD: &str = "general";

pub fn bootstrap_anchor() -> (AnchorKind, String, Option<String>) {
    (AnchorKind::Repo, LEGACY_REPO_ANCHOR_ID.to_string(), None)
}

pub fn bootstrap_provenance(source_type: &SourceType) -> Provenance {
    match source_type {
        SourceType::Project => Provenance::Research,
        SourceType::Conversation | SourceType::Manual => Provenance::Human,
    }
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
