use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Project,
    Conversation,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Evidence,
    Knowledge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDomain {
    Project,
    Agent,
    Skill,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    Global,
    Repo,
    Worktree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Runtime,
    Research,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeTier {
    Qi,
    Shu,
    DaoRen,
    DaoTian,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeStatus {
    Candidate,
    Promoted,
    Canonical,
    Demoted,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerHints {
    pub intent_tags: Vec<String>,
    pub workflow_bias: Vec<String>,
    pub tool_needs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drawer {
    pub id: String,
    pub content: String,
    pub wing: String,
    pub room: Option<String>,
    pub source_file: Option<String>,
    pub source_type: SourceType,
    pub added_at: String,
    pub chunk_index: Option<i64>,
    /// Importance ranking (0-5). Higher = more important for wake-up context.
    #[serde(default)]
    pub importance: i32,
    pub memory_kind: MemoryKind,
    pub domain: MemoryDomain,
    pub field: String,
    pub anchor_kind: AnchorKind,
    pub anchor_id: String,
    pub parent_anchor_id: Option<String>,
    pub provenance: Option<Provenance>,
    pub statement: Option<String>,
    pub tier: Option<KnowledgeTier>,
    pub status: Option<KnowledgeStatus>,
    #[serde(default)]
    pub supporting_refs: Vec<String>,
    #[serde(default)]
    pub counterexample_refs: Vec<String>,
    #[serde(default)]
    pub teaching_refs: Vec<String>,
    #[serde(default)]
    pub verification_refs: Vec<String>,
    pub scope_constraints: Option<String>,
    pub trigger_hints: Option<TriggerHints>,
}

impl Drawer {
    pub fn new_bootstrap_evidence(
        id: String,
        content: String,
        wing: String,
        room: Option<String>,
        source_file: Option<String>,
        source_type: SourceType,
        added_at: String,
        chunk_index: Option<i64>,
        importance: i32,
    ) -> Self {
        let provenance = match source_type {
            SourceType::Project => Some(Provenance::Research),
            SourceType::Conversation | SourceType::Manual => Some(Provenance::Human),
        };

        Self {
            id,
            content,
            wing,
            room,
            source_file,
            source_type,
            added_at,
            chunk_index,
            importance,
            memory_kind: MemoryKind::Evidence,
            domain: MemoryDomain::Project,
            field: "general".to_string(),
            anchor_kind: AnchorKind::Repo,
            anchor_id: "repo://legacy".to_string(),
            parent_anchor_id: None,
            provenance,
            statement: None,
            tier: None,
            status: None,
            supporting_refs: Vec::new(),
            counterexample_refs: Vec::new(),
            teaching_refs: Vec::new(),
            verification_refs: Vec::new(),
            scope_constraints: None,
            trigger_hints: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Triple {
    pub id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub confidence: f64,
    pub source_drawer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyEntry {
    pub wing: String,
    pub room: String,
    pub display_name: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TripleStats {
    pub total: i64,
    pub active: i64,
    pub expired: i64,
    pub entities: i64,
    pub top_predicates: Vec<(String, i64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteDecision {
    pub wing: Option<String>,
    pub room: Option<String>,
    pub confidence: f32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub drawer_id: String,
    pub content: String,
    pub wing: String,
    pub room: Option<String>,
    pub source_file: String,
    pub similarity: f32,
    pub route: RouteDecision,
    /// Other wings that share this result's room (tunnel hints).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tunnel_hints: Vec<String>,
}
