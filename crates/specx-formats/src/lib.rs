//! Pure format helpers for import, export, and preview tooling.
//!
//! Format code is intentionally independent from `specx-core`: it transforms
//! content but never loads resources, opens a database, or performs a request.

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
    use super::{normalize_line_endings, sanitize_markdown};

    #[test]
    fn normalizes_cross_platform_line_endings() {
        assert_eq!(normalize_line_endings("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn strips_executable_markdown_fragments() {
        assert_eq!(sanitize_markdown("safe <ScRiPt>alert(1)</sCrIpT> JaVaScRiPt:run"), "safe  run");
    }
}
