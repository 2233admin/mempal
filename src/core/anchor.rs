use super::types::{AnchorKind, MemoryDomain, Provenance, SourceType};

pub const LEGACY_REPO_ANCHOR_ID: &str = "repo://legacy";
pub const DEFAULT_FIELD: &str = "general";

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

pub fn validate_anchor_domain(
    domain: &MemoryDomain,
    anchor_kind: &AnchorKind,
) -> Result<(), &'static str> {
    if matches!(anchor_kind, AnchorKind::Global) && !matches!(domain, MemoryDomain::Global) {
        return Err("global anchor requires domain=global");
    }
    Ok(())
}
