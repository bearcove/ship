use std::sync::Arc;

use facet_json::RawJson;

/// Arbitrary extension request not part of the ACP spec.
#[derive(Debug, Clone)]
pub struct ExtRequest {
    pub method: Arc<str>,
    pub params: RawJson<'static>,
}

impl ExtRequest {
    pub fn new(method: impl Into<Arc<str>>, params: RawJson<'static>) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }
}

/// Arbitrary extension response.
#[derive(Debug, Clone)]
pub struct ExtResponse(pub RawJson<'static>);

impl ExtResponse {
    pub fn new(params: RawJson<'static>) -> Self {
        Self(params)
    }
}

/// Arbitrary extension notification.
#[derive(Debug, Clone)]
pub struct ExtNotification {
    pub method: Arc<str>,
    pub params: RawJson<'static>,
}

impl ExtNotification {
    pub fn new(method: impl Into<Arc<str>>, params: RawJson<'static>) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }
}
