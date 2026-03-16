use facet::Facet;

/// An execution plan for complex tasks.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Plan {
    pub entries: Vec<PlanEntry>,
}

impl Plan {
    pub fn new(entries: Vec<PlanEntry>) -> Self {
        Self { entries }
    }
}

/// A single entry in the execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanEntryPriority,
    pub status: PlanEntryStatus,
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
