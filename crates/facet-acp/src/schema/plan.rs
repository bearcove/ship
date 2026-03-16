use facet::Facet;
use facet_json::RawJson;

/// An execution plan for complex tasks.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Plan {
    pub entries: Vec<PlanEntry>,
    #[facet(default, skip_unless_truthy, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl Plan {
    pub fn new(entries: Vec<PlanEntry>) -> Self {
        Self {
            entries,
            meta: None,
        }
    }
}

/// A single entry in the execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanEntryPriority,
    pub status: PlanEntryStatus,
    #[facet(default, skip_unless_truthy, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl PlanEntry {
    pub fn new(
        content: impl Into<String>,
        priority: PlanEntryPriority,
        status: PlanEntryStatus,
    ) -> Self {
        Self {
            content: content.into(),
            priority,
            status,
            meta: None,
        }
    }
}

/// Priority levels for plan entries.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

/// Status of a plan entry.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
}
