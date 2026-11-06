//! Shared API/host error catalog. Core remains independent of HTTP and locale.

use std::collections::BTreeMap;

use axum::{
    Json,
    extract::Request,
    http::{
        StatusCode,
        header::{CONTENT_LENGTH, CONTENT_TYPE},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use specx_core::CoreError;

/// Versioned error envelope. Fields are constructed only by the catalog.
#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    #[schema(minimum = 1, maximum = 1)]
    version: u8,
    code: &'static str,
    message_key: &'static str,
    /// Parameters are JSON scalars selected by the catalog entry. The current
    /// catalog is parameterless, but this keeps future interpolation typed.
    message_params: BTreeMap<String, serde_json::Value>,
}

macro_rules! error_catalog {
    ($($kind:ident => ($status:literal, $code:literal, $key:literal),)*) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ErrorKind { $($kind,)* }

        impl ErrorKind {
            pub const ALL: &'static [Self] = &[$(Self::$kind,)*];

            pub fn status(self) -> StatusCode {
                match self {
                    $(Self::$kind => StatusCode::from_u16($status).expect("catalog status is valid"),)*
                }
            }

            pub fn body(self) -> ErrorResponse {
                let (code, message_key) = match self {
                    $(Self::$kind => ($code, $key),)*
                };
                ErrorResponse {
                    version: 1, code, message_key, message_params: BTreeMap::new(),
                }
            }
        }
    };
}

// Each entry owns one stable machine code and one localized summary key.
error_catalog! {
    InvalidCredentials => (401, "invalid_credentials", "apiError.invalidCredentials"),
    Unauthorized => (401, "unauthorized", "apiError.unauthorized"),
    Forbidden => (403, "forbidden", "apiError.forbidden"),
    ScopeMismatch => (403, "scope_mismatch", "apiError.scopeMismatch"),
    NotFound => (404, "not_found", "apiError.notFound"),
    OriginRejected => (403, "origin_rejected", "apiError.originRejected"),
    RateLimited => (429, "rate_limited", "apiError.rateLimited"),
    NotReady => (503, "not_ready", "apiError.notReady"),
    InvalidQuery => (400, "invalid_query", "apiError.invalidQuery"),
    InvalidInput => (422, "invalid_input", "apiError.invalidInput"),
    Conflict => (409, "request_conflict", "apiError.conflict"),
    LastOwnerRequired => (409, "last_owner_required", "apiError.lastOwnerRequired"),
    IdempotencyConflict => (409, "idempotency_conflict", "apiError.idempotencyConflict"),
    PreconditionRequired => (428, "precondition_required", "apiError.preconditionRequired"),
    PreconditionFailed => (412, "precondition_failed", "apiError.preconditionFailed"),
    EmptyName => (422, "empty_name", "apiError.emptyName"),
    EmptySlug => (422, "empty_slug", "apiError.emptySlug"),
    EmptyContent => (422, "empty_content", "apiError.emptyContent"),
    InvalidParent => (422, "invalid_parent", "apiError.invalidParent"),
    GroupCycle => (409, "group_cycle", "apiError.groupCycle"),
    InvalidGroupOrder => (422, "invalid_group_order", "apiError.invalidGroupOrder"),
    DuplicateTag => (409, "duplicate_tag", "apiError.duplicateTag"),
    DuplicateSlug => (409, "duplicate_slug", "apiError.duplicateSlug"),
    DuplicateCodeSource => (409, "duplicate_code_source", "apiError.duplicateCodeSource"),
    DuplicateCodeBinding => (409, "duplicate_code_binding", "apiError.duplicateCodeBinding"),
    InvalidCodeSource => (422, "invalid_code_source", "apiError.invalidCodeSource"),
    InvalidCodeBinding => (422, "invalid_code_binding", "apiError.invalidCodeBinding"),
    HasDependents => (409, "has_dependents", "apiError.hasDependents"),
    Internal => (500, "internal_error", "apiError.internal"),
    InvalidRequest => (400, "invalid_request", "apiError.invalidRequest"),
    MethodNotAllowed => (405, "method_not_allowed", "apiError.methodNotAllowed"),
    PayloadTooLarge => (413, "payload_too_large", "apiError.payloadTooLarge"),
    UnsupportedMediaType => (415, "unsupported_media_type", "apiError.unsupportedMediaType"),
}

impl From<CoreError> for ErrorKind {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::EmptyName => Self::EmptyName,
            CoreError::EmptySlug => Self::EmptySlug,
            CoreError::EmptyContent => Self::EmptyContent,
            CoreError::NotFound(_) => Self::NotFound,
            CoreError::ScopeMismatch => Self::ScopeMismatch,
            CoreError::InvalidParent => Self::InvalidParent,
            CoreError::GroupCycle => Self::GroupCycle,
            CoreError::InvalidGroupOrder => Self::InvalidGroupOrder,
            CoreError::DuplicateTag => Self::DuplicateTag,
            CoreError::DuplicateSlug => Self::DuplicateSlug,
            CoreError::DuplicateCodeSource => Self::DuplicateCodeSource,
            CoreError::DuplicateCodeBinding => Self::DuplicateCodeBinding,
            CoreError::InvalidCodeSource => Self::InvalidCodeSource,
            CoreError::InvalidCodeBinding => Self::InvalidCodeBinding,
            CoreError::HasDependents(_) => Self::HasDependents,
        }
    }
}

#[derive(Clone)]
struct CatalogResponse;

impl IntoResponse for ErrorKind {
    fn into_response(self) -> Response {
        let mut response = (self.status(), Json(self.body())).into_response();
        response.extensions_mut().insert(CatalogResponse);
        response
    }
}

/// Normalize framework/extractor rejections without reading or exposing their
/// bodies. Preserve statuses and headers such as Allow and x-request-id.
pub async fn normalize_error_response(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error())
        || response.extensions().get::<CatalogResponse>().is_some()
    {
        return response;
    }
    let kind = match status {
        StatusCode::BAD_REQUEST => ErrorKind::InvalidRequest,
        StatusCode::UNAUTHORIZED => ErrorKind::Unauthorized,
        StatusCode::FORBIDDEN => ErrorKind::Forbidden,
        StatusCode::CONFLICT => ErrorKind::Conflict,
        StatusCode::PRECONDITION_FAILED => ErrorKind::PreconditionFailed,
        StatusCode::PRECONDITION_REQUIRED => ErrorKind::PreconditionRequired,
        StatusCode::TOO_MANY_REQUESTS => ErrorKind::RateLimited,
        StatusCode::SERVICE_UNAVAILABLE => ErrorKind::NotReady,
        StatusCode::NOT_FOUND => ErrorKind::NotFound,
        StatusCode::METHOD_NOT_ALLOWED => ErrorKind::MethodNotAllowed,
        StatusCode::PAYLOAD_TOO_LARGE => ErrorKind::PayloadTooLarge,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => ErrorKind::UnsupportedMediaType,
        StatusCode::UNPROCESSABLE_ENTITY => ErrorKind::InvalidInput,
        _ if status.is_server_error() => ErrorKind::Internal,
        _ => ErrorKind::InvalidInput,
    };
    let mut normalized = kind.into_response();
    *normalized.status_mut() = status;
    for (name, value) in response.headers() {
        if name != CONTENT_TYPE && name != CONTENT_LENGTH {
            normalized.headers_mut().append(name, value.clone());
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_matches_the_cross_language_fixture() {
        let entries: Vec<_> = ErrorKind::ALL
            .iter()
            .map(|kind| serde_json::json!({ "status": kind.status().as_u16(), "body": kind.body() }))
            .collect();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/error-catalog.json")).unwrap();
        assert_eq!(serde_json::json!(entries), fixture);
        let keys: BTreeSet<_> = entries.iter().map(|entry| entry["body"]["messageKey"].as_str().unwrap()).collect();
        let codes: BTreeSet<_> = entries.iter().map(|entry| entry["body"]["code"].as_str().unwrap()).collect();
        assert_eq!(keys.len(), entries.len(), "catalog keys must be unique");
        assert_eq!(codes.len(), entries.len(), "catalog codes must be unique");
    }
}
