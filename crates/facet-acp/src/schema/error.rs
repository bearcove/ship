use facet::Facet;
use facet_json::RawJson;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// JSON-RPC error object.
#[derive(Debug, Clone, PartialEq, Eq, Facet)]
pub struct Error {
    pub code: i32,
    pub message: String,
    #[facet(default, skip_unless_truthy)]
    pub data: Option<RawJson<'static>>,
}

impl Error {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Error {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn data(mut self, data: impl Into<String>) -> Self {
        let s = data.into();
        let json_string = facet_json::to_string(&s).unwrap_or_else(|_| format!("\"{}\"", s));
        self.data = Some(RawJson::from_owned(json_string));
        self
    }

    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid request")
    }

    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }

    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }

    pub fn internal_error() -> Self {
        Self::new(-32603, "Internal error")
    }

    pub fn request_cancelled() -> Self {
        Self::new(-32800, "Request cancelled")
    }

    pub fn auth_required() -> Self {
        Self::new(-32000, "Authentication required")
    }

    pub fn resource_not_found(uri: Option<String>) -> Self {
        let err = Self::new(-32002, "Resource not found");
        if let Some(uri) = uri {
            err.data(format!("uri: {uri}"))
        } else {
            err
        }
    }

    pub fn into_internal_error(err: impl std::error::Error) -> Self {
        Error::internal_error().data(err.to_string())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(data) = &self.data {
            write!(f, ": {}", data.as_ref())?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}
