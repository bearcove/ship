use facet::Facet;

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

/// Text provided to or from an LLM.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct TextContent {
    pub text: String,
}

impl TextContent {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// An image provided to or from an LLM.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ImageContent {
    pub data: String,
    pub mime_type: String,
}

impl ImageContent {
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }
}

/// Audio provided to or from an LLM.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct AudioContent {
    pub data: String,
    pub mime_type: String,
}

impl AudioContent {
    pub fn new(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }
}

/// Complete resource contents embedded in a message.
#[derive(Debug, Clone, PartialEq, Facet)]
pub struct EmbeddedResource {
    pub resource: EmbeddedResourceResource,
}

impl EmbeddedResource {
    pub fn new(resource: EmbeddedResourceResource) -> Self {
        Self { resource }
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
    pub text: String,
    pub uri: String,
    #[facet(default)]
    pub mime_type: Option<String>,
}

impl TextResourceContents {
    pub fn new(text: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            uri: uri.into(),
            mime_type: None,
        }
    }
}

/// Binary resource contents.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct BlobResourceContents {
    pub blob: String,
    pub uri: String,
    #[facet(default)]
    pub mime_type: Option<String>,
}

impl BlobResourceContents {
    pub fn new(blob: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            blob: blob.into(),
            uri: uri.into(),
            mime_type: None,
        }
    }
}

/// A resource link.
#[derive(Debug, Clone, PartialEq, Facet)]
#[facet(rename_all = "camelCase")]
pub struct ResourceLink {
    pub name: String,
    pub uri: String,
    #[facet(default)]
    pub description: Option<String>,
    #[facet(default)]
    pub mime_type: Option<String>,
    #[facet(default)]
    pub size: Option<i64>,
    #[facet(default)]
    pub title: Option<String>,
}

impl ResourceLink {
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uri: uri.into(),
            description: None,
            mime_type: None,
            size: None,
            title: None,
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
}

impl ContentChunk {
    pub fn new(content: ContentBlock) -> Self {
        Self { content }
    }
}
