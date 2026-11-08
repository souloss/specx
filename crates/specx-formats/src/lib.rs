//! Pure format helpers for import, export, and preview tooling.
//!
//! Format code is intentionally independent from `specx-core`: it transforms
//! content but never loads resources, opens a database, or performs a request.

use serde_json::Value;
use std::fmt;

const MAX_OPENAPI_BYTES: usize = 5 * 1024 * 1024;
const MAX_OPENAPI_OPERATIONS: usize = 10_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenApiDocument {
    pub version: String,
    pub title: Option<String>,
    pub document_version: Option<String>,
    pub servers: Vec<String>,
    pub tags: Vec<String>,
    pub operations: Vec<OpenApiOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenApiOperation {
    pub method: String,
    pub path: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub identity_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenApiValidationError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for OpenApiValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for OpenApiValidationError {}

/// Parse and validate an OpenAPI 3 document without resolving network references.
pub fn parse_openapi(value: &str) -> Result<OpenApiDocument, OpenApiValidationError> {
    if value.len() > MAX_OPENAPI_BYTES {
        return Err(validation("$", "document exceeds the 5 MiB limit"));
    }
    let document: Value = serde_json::from_str(value)
        .or_else(|_| {
            serde_yaml::from_str::<serde_yaml::Value>(value)
                .map_err(|error| serde_json::Error::io(std::io::Error::other(error.to_string())))
                .and_then(|yaml| {
                    serde_json::to_value(yaml)
                        .map_err(|error| serde_json::Error::io(std::io::Error::other(error.to_string())))
                })
        })
        .map_err(|error| validation("$", &format!("invalid JSON/YAML: {error}")))?;
    parse_openapi_value(document)
}

fn parse_openapi_value(document: Value) -> Result<OpenApiDocument, OpenApiValidationError> {
    let object = document.as_object().ok_or_else(|| validation("$", "document must be an object"))?;
    reject_external_refs(&document, "$", 0)?;
    let version =
        object.get("openapi").and_then(Value::as_str).ok_or_else(|| validation("$.openapi", "must be a string"))?;
    if !(version.starts_with("3.0.") || version.starts_with("3.1.")) {
        return Err(validation("$.openapi", "only OpenAPI 3.0.x and 3.1.x are supported"));
    }
    let info =
        object.get("info").and_then(Value::as_object).ok_or_else(|| validation("$.info", "must be an object"))?;
    let title = info.get("title").and_then(Value::as_str).map(str::to_owned);
    let document_version = info.get("version").and_then(Value::as_str).map(str::to_owned);
    let servers = object
        .get("servers")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().filter_map(|item| item.get("url").and_then(Value::as_str).map(str::to_owned)).collect()
        })
        .unwrap_or_default();
    let tags = object
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_owned)).collect()
        })
        .unwrap_or_default();
    let paths =
        object.get("paths").and_then(Value::as_object).ok_or_else(|| validation("$.paths", "must be an object"))?;
    let methods = ["get", "put", "post", "delete", "options", "head", "patch", "trace"];
    let mut operations = Vec::new();
    for (path, item) in paths {
        if !path.starts_with('/') {
            return Err(validation(&format!("$.paths.{path}"), "path must start with '/'"));
        }
        let Some(path_item) = item.as_object() else {
            return Err(validation(&format!("$.paths.{path}"), "must be an object"));
        };
        for method in methods {
            let Some(operation) = path_item.get(method) else { continue };
            let Some(operation) = operation.as_object() else {
                return Err(validation(&format!("$.paths.{path}.{method}"), "must be an object"));
            };
            let tags_for_operation = operation
                .get("tags")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default();
            operations.push(OpenApiOperation {
                method: method.to_ascii_uppercase(),
                path: path.clone(),
                operation_id: operation.get("operationId").and_then(Value::as_str).map(str::to_owned),
                summary: operation.get("summary").and_then(Value::as_str).map(str::to_owned),
                description: operation.get("description").and_then(Value::as_str).map(str::to_owned),
                tags: tags_for_operation,
                identity_key: format!("{}:{}", method.to_ascii_uppercase(), path),
            });
            if operations.len() > MAX_OPENAPI_OPERATIONS {
                return Err(validation("$.paths", "operation count exceeds the 10,000 limit"));
            }
        }
    }
    operations.sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));
    Ok(OpenApiDocument { version: version.to_owned(), title, document_version, servers, tags, operations })
}

fn reject_external_refs(value: &Value, path: &str, depth: usize) -> Result<(), OpenApiValidationError> {
    if depth > 64 {
        return Err(validation(path, "reference nesting exceeds the limit"));
    }
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && !reference.starts_with("#/")
            {
                return Err(validation(&format!("{path}.$ref"), "external references are not supported"));
            }
            for (key, child) in object {
                reject_external_refs(child, &format!("{path}.{key}"), depth + 1)?;
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_external_refs(child, &format!("{path}[{index}]"), depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validation(path: &str, message: &str) -> OpenApiValidationError {
    OpenApiValidationError { path: path.to_owned(), message: message.to_owned() }
}

/// Normalize line endings so generated documents are deterministic across OSes.
pub fn normalize_line_endings(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n")
}

/// Remove the two executable constructs that the Markdown preview does not
/// support. Full Markdown rendering remains a host/UI responsibility.
pub fn sanitize_markdown(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(start) = remaining.to_ascii_lowercase().find("<script") {
        let (before, script) = remaining.split_at(start);
        result.push_str(before);
        let Some(end) = script.to_ascii_lowercase().find("</script>") else {
            remaining = "";
            break;
        };
        remaining = &script[end + "</script>".len()..];
    }
    result.push_str(remaining);
    strip_ascii_case_insensitive(&result, "javascript:")
}

fn strip_ascii_case_insensitive(value: &str, needle: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(index) = remaining.to_ascii_lowercase().find(needle) {
        let (before, matched) = remaining.split_at(index);
        output.push_str(before);
        remaining = &matched[needle.len()..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::{normalize_line_endings, parse_openapi, sanitize_markdown};

    #[test]
    fn normalizes_cross_platform_line_endings() {
        assert_eq!(normalize_line_endings("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn strips_executable_markdown_fragments() {
        assert_eq!(sanitize_markdown("safe <ScRiPt>alert(1)</sCrIpT> JaVaScRiPt:run"), "safe  run");
    }

    #[test]
    fn parses_json_openapi_and_stable_operations() {
        let document = parse_openapi(r#"{"openapi":"3.0.3","info":{"title":"Demo","version":"1"},"paths":{"/users":{"get":{"tags":["Users"],"summary":"List"}}}}"#).unwrap();
        assert_eq!(document.operations[0].identity_key, "GET:/users");
        assert_eq!(document.operations[0].tags, ["Users"]);
    }

    #[test]
    fn parses_yaml_and_rejects_external_refs() {
        let yaml = "openapi: 3.1.0\ninfo:\n  title: Demo\n  version: '1'\npaths: {}\n";
        assert_eq!(parse_openapi(yaml).unwrap().version, "3.1.0");
        let external = r#"{"openapi":"3.0.0","info":{"title":"x","version":"1"},"paths":{},"components":{"schemas":{"X":{"$ref":"https://example.test/x"}}}}"#;
        assert!(parse_openapi(external).is_err());
    }
}
