use facet::Facet;
use facet_json::RawJson;

/// Content blocks represent displayable information in ACP.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(tag = "type", rename_all = "snake_case")]
#[repr(u8)]
pub enum ContentBlock {
    /// Text content, plain or markdown.
    Text(TextContent),
    /// Image content, base64-encoded.
    Image(ImageContent),
    /// Audio content, base64-encoded.
    Audio(AudioContent),
    /// Reference to a resource the agent can access.
    ResourceLink(ResourceLink),
    /// Complete resource contents embedded directly.
    Resource(EmbeddedResource),
}

impl<T: Into<String>> From<T> for ContentBlock {
    fn from(value: T) -> Self {
        Self::Text(TextContent::new(value))
    }
}

/// Optional annotations for the client.
#[derive(Debug, Clone, PartialEq, Default, Facet)]
#[facet(rename_all = "camelCase")]
pub struct Annotations {
    #[facet(default)]
    pub audience: Option<Vec<Role>>,
    #[facet(default)]
    pub last_modified: Option<String>,
    #[facet(default)]
    pub priority: Option<f64>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

/// Text provided to or from an LLM.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TextContent {
    #[facet(default)]
    pub annotations: Option<Annotations>,
    pub text: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl TextContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            annotations: None,
            text: text.into(),
            meta: None,
        }
    }
}

/// An image provided to or from an LLM.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ImageContent {
    #[facet(default)]
    pub annotations: Option<Annotations>,
    pub data: String,
    pub mime_type: String,
    #[facet(default)]
    pub uri: Option<String>,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl ImageContent {
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            annotations: None,
            data: data.into(),
            mime_type: mime_type.into(),
            uri: None,
            meta: None,
        }
    }
}

/// Audio provided to or from an LLM.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AudioContent {
    #[facet(default)]
    pub annotations: Option<Annotations>,
    pub data: String,
    pub mime_type: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl AudioContent {
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            annotations: None,
            data: data.into(),
            mime_type: mime_type.into(),
            meta: None,
        }
    }
}

/// Complete resource contents embedded in a message.
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct EmbeddedResource {
    #[facet(default)]
    pub annotations: Option<Annotations>,
    pub resource: EmbeddedResourceResource,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl EmbeddedResource {
    pub fn new(resource: EmbeddedResourceResource) -> Self {
        Self {
            annotations: None,
            resource,
            meta: None,
        }
    }
}

/// Resource content that can be embedded.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(untagged)]
#[repr(u8)]
pub enum EmbeddedResourceResource {
    TextResourceContents(TextResourceContents),
    BlobResourceContents(BlobResourceContents),
}

/// Text-based resource contents.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TextResourceContents {
    #[facet(default)]
    pub mime_type: Option<String>,
    pub text: String,
    pub uri: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl TextResourceContents {
    pub fn new(text: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            mime_type: None,
            text: text.into(),
            uri: uri.into(),
            meta: None,
        }
    }
}

/// Binary resource contents.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct BlobResourceContents {
    pub blob: String,
    #[facet(default)]
    pub mime_type: Option<String>,
    pub uri: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl BlobResourceContents {
    pub fn new(blob: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            blob: blob.into(),
            mime_type: None,
            uri: uri.into(),
            meta: None,
        }
    }
}

/// A resource link.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ResourceLink {
    #[facet(default)]
    pub annotations: Option<Annotations>,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(default)]
    pub mime_type: Option<String>,
    pub name: String,
    #[facet(default)]
    pub size: Option<i64>,
    #[facet(default)]
    pub title: Option<String>,
    pub uri: String,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl ResourceLink {
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            annotations: None,
            description: None,
            mime_type: None,
            name: name.into(),
            size: None,
            title: None,
            uri: uri.into(),
            meta: None,
        }
    }
}

/// The sender or recipient of messages and data.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
#[facet(rename_all = "camelCase")]
#[repr(u8)]
pub enum Role {
    Assistant,
    User,
}

/// A streamed item of content.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ContentChunk {
    pub content: ContentBlock,
    #[facet(default, rename = "_meta")]
    pub meta: Option<RawJson<'static>>,
}

impl ContentChunk {
    pub fn new(content: ContentBlock) -> Self {
        Self {
            content,
            meta: None,
        }
    }
}
