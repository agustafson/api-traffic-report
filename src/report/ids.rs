//! Newtypes for the domain identifiers carried through the report.

use serde::Serialize;

/// The `client_id` a request log attributes to a client.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(super) struct ClientId(String);

impl ClientId {
    pub(super) fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// The API path a request was made against.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(super) struct Endpoint(String);

impl Endpoint {
    pub(super) fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }
}

/// An HTTP status code, already validated to fall within 100-599.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub(super) struct StatusCode(u16);

impl StatusCode {
    pub(super) fn new(code: u16) -> Self {
        Self(code)
    }
}
