//! XRPC request and response shapes, independent of the HTTP transport.

use serde_json::{Value, json};

use crate::error::Rejection;

/// Insert `key` into a JSON object (lexicon views are always objects).
pub(crate) fn set_field(value: &mut Value, key: &str, field: Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_string(), field);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XrpcRequest {
    pub method: Method,
    /// The NSID after `/xrpc/`.
    pub nsid: String,
    /// Decoded query parameters in request order.
    pub query: Vec<(String, String)>,
    pub authorization: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XrpcResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl XrpcResponse {
    #[must_use]
    pub fn json(status: u16, value: &Value) -> Self {
        // Encoding a `Value` cannot fail; an empty object is the safe fallback.
        let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
        Self { status, body }
    }

    #[must_use]
    pub fn ok(value: &Value) -> Self {
        Self::json(200, value)
    }

    // r[impl kiln.tangled.xrpc]
    /// XRPC error envelope `{error, message}`; the Kiln code is the message prefix.
    #[must_use]
    pub fn error(rejection: &Rejection) -> Self {
        let (status, name) = rejection.status();
        Self::json(
            status,
            &json!({ "error": name, "message": format!("{}: {}", rejection.code, rejection.message) }),
        )
    }

    #[must_use]
    pub fn named(status: u16, name: &str, message: &str) -> Self {
        Self::json(status, &json!({ "error": name, "message": message }))
    }
}

impl XrpcRequest {
    /// The single value of a query parameter; duplicates are refused.
    pub fn param(&self, name: &str) -> Result<Option<&str>, Rejection> {
        let mut values = self
            .query
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_str());
        let first = values.next();
        if values.next().is_some() {
            return Err(Rejection::invalid(
                "tangled_query_duplicate",
                "Query parameter is repeated",
            ));
        }
        Ok(first)
    }

    pub fn required(&self, name: &str) -> Result<&str, Rejection> {
        self.param(name)?
            .filter(|value| !value.is_empty())
            .ok_or(Rejection::invalid(
                "tangled_query_missing",
                "Required query parameter is missing",
            ))
    }
}
