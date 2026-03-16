use std::path::PathBuf;
use std::sync::Arc;

use facet::Facet;
use facet_json::RawJson;

use crate::ContentBlock;

/// Unique identifier for a tool call within a session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct ToolCallId(pub Arc<str>);

impl ToolCallId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl std::fmt::Display for ToolCallId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ToolCallId {
    fn from(s: String) -> Self {
        Self(Arc::from(s.as_str()))
    }
}

impl From<&str> for ToolCallId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

/// Represents a tool call that the language model has requested.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ToolCall {
    pub tool_call_id: ToolCallId,
    pub title: String,
    #[facet(default)]
    pub kind: ToolKind,
    #[facet(default)]
    pub status: ToolCallStatus,
    #[facet(default)]
    pub content: Vec<ToolCallContent>,
    #[facet(default)]
    pub locations: Vec<ToolCallLocation>,
    #[facet(default)]
    pub raw_input: Option<RawJson<'static>>,
    #[facet(default)]
    pub raw_output: Option<RawJson<'static>>,
}

impl ToolCall {
    pub fn new(tool_call_id: impl Into<ToolCallId>, title: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            title: title.into(),
            kind: ToolKind::default(),
            status: ToolCallStatus::default(),
            content: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            raw_output: None,
        }
    }

    pub fn kind(mut self, kind: ToolKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn status(mut self, status: ToolCallStatus) -> Self {
        self.status = status;
        self
    }

    pub fn content(mut self, content: Vec<ToolCallContent>) -> Self {
        self.content = content;
        self
    }

    pub fn locations(mut self, locations: Vec<ToolCallLocation>) -> Self {
        self.locations = locations;
        self
    }

    /// Update an existing tool call with the values in the provided update fields.
    pub fn update(&mut self, fields: ToolCallUpdateFields) {
        if let Some(title) = fields.title {
            self.title = title;
        }
        if let Some(kind) = fields.kind {
            self.kind = kind;
        }
        if let Some(status) = fields.status {
            self.status = status;
        }
        if let Some(content) = fields.content {
            self.content = content;
        }
        if let Some(locations) = fields.locations {
            self.locations = locations;
        }
        if let Some(raw_input) = fields.raw_input {
            self.raw_input = Some(raw_input);
        }
        if let Some(raw_output) = fields.raw_output {
            self.raw_output = Some(raw_output);
        }
    }
}

/// An update to an existing tool call.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ToolCallUpdate {
    pub tool_call_id: ToolCallId,
    // TODO: facet doesn't support #[facet(flatten)] yet, so we inline the fields
    #[facet(default)]
    pub kind: Option<ToolKind>,
    #[facet(default)]
    pub status: Option<ToolCallStatus>,
    #[facet(default)]
    pub title: Option<String>,
    #[facet(default)]
    pub content: Option<Vec<ToolCallContent>>,
    #[facet(default)]
    pub locations: Option<Vec<ToolCallLocation>>,
    #[facet(default)]
    pub raw_input: Option<RawJson<'static>>,
    #[facet(default)]
    pub raw_output: Option<RawJson<'static>>,
}

impl ToolCallUpdate {
    pub fn new(tool_call_id: impl Into<ToolCallId>, fields: ToolCallUpdateFields) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            kind: fields.kind,
            status: fields.status,
            title: fields.title,
            content: fields.content,
            locations: fields.locations,
            raw_input: fields.raw_input,
            raw_output: fields.raw_output,
        }
    }

    /// Extract the update fields (everything except the ID).
    pub fn into_fields(self) -> ToolCallUpdateFields {
        ToolCallUpdateFields {
            kind: self.kind,
            status: self.status,
            title: self.title,
            content: self.content,
            locations: self.locations,
            raw_input: self.raw_input,
            raw_output: self.raw_output,
        }
    }
}

/// Optional fields that can be updated in a tool call.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct ToolCallUpdateFields {
    pub kind: Option<ToolKind>,
    pub status: Option<ToolCallStatus>,
    pub title: Option<String>,
    pub content: Option<Vec<ToolCallContent>>,
    pub locations: Option<Vec<ToolCallLocation>>,
    pub raw_input: Option<RawJson<'static>>,
    pub raw_output: Option<RawJson<'static>>,
}

impl ToolCallUpdateFields {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Categories of tools that can be invoked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum ToolKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    SwitchMode,
    #[default]
    Other,
}

/// Execution status of a tool call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Facet)]
#[facet(rename_all = "snake_case")]
#[repr(u8)]
pub enum ToolCallStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Content produced by a tool call.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(tag = "type", rename_all = "snake_case")]
#[repr(u8)]
pub enum ToolCallContent {
    Content(Content),
    Diff(Diff),
    Terminal(Terminal),
}

impl<T: Into<ContentBlock>> From<T> for ToolCallContent {
    fn from(content: T) -> Self {
        ToolCallContent::Content(Content::new(content))
    }
}

/// Standard content block wrapper.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Content {
    pub content: ContentBlock,
}

impl Content {
    pub fn new(content: impl Into<ContentBlock>) -> Self {
        Self {
            content: content.into(),
        }
    }
}

/// Embed a terminal by its id.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Terminal {
    pub terminal_id: TerminalId,
}

impl Terminal {
    pub fn new(terminal_id: impl Into<TerminalId>) -> Self {
        Self {
            terminal_id: terminal_id.into(),
        }
    }
}

/// Unique identifier for a terminal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Facet)]
#[facet(transparent)]
pub struct TerminalId(pub Arc<str>);

impl TerminalId {
    pub fn new(id: impl Into<Arc<str>>) -> Self {
        Self(id.into())
    }
}

impl From<String> for TerminalId {
    fn from(s: String) -> Self {
        Self(Arc::from(s.as_str()))
    }
}

impl From<&str> for TerminalId {
    fn from(s: &str) -> Self {
        Self(Arc::from(s))
    }
}

/// A diff representing file modifications.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Diff {
    pub path: PathBuf,
    #[facet(default)]
    pub old_text: Option<String>,
    pub new_text: String,
}

impl Diff {
    pub fn new(path: impl Into<PathBuf>, new_text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            old_text: None,
            new_text: new_text.into(),
        }
    }

    pub fn old_text(mut self, old_text: impl Into<String>) -> Self {
        self.old_text = Some(old_text.into());
        self
    }
}

/// A file location being accessed or modified by a tool.
#[derive(Clone, Debug, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ToolCallLocation {
    pub path: PathBuf,
    #[facet(default)]
    pub line: Option<u32>,
}

impl ToolCallLocation {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            line: None,
        }
    }

    pub fn line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }
}
