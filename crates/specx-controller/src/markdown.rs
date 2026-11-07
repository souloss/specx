use crate::{GitFileRequest, GitProvider, GitProviderError};
use sha2::{Digest, Sha256};
use specx_formats::normalize_line_endings;
use specx_types::SpecificationRefKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownDocumentRequest {
    pub repository_url: String,
    pub ref_kind: SpecificationRefKind,
    pub ref_name: String,
    pub path: String,
    pub current_content_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownDocumentResult {
    pub content: String,
    pub content_hash: String,
    pub resolved_commit: String,
    pub manifest_hash: String,
    pub provider_version: String,
    pub changed: bool,
}

pub struct MarkdownDocumentController<P> {
    provider: P,
}

impl<P> MarkdownDocumentController<P>
where
    P: GitProvider,
{
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn reconcile(&self, request: &MarkdownDocumentRequest) -> Result<MarkdownDocumentResult, GitProviderError> {
        let snapshot = self.provider.read_markdown_file(&GitFileRequest {
            repository_url: request.repository_url.clone(),
            ref_kind: request.ref_kind,
            ref_name: request.ref_name.clone(),
            path: request.path.clone(),
        })?;
        let content = normalize_line_endings(&snapshot.content);
        let digest = Sha256::digest(content.as_bytes());
        let content_hash = format!("sha256:{digest:x}");
        Ok(MarkdownDocumentResult {
            changed: request.current_content_hash.as_deref() != Some(content_hash.as_str()),
            content,
            content_hash,
            resolved_commit: snapshot.resolved_commit,
            manifest_hash: snapshot.manifest_hash,
            provider_version: snapshot.provider_version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GitFileRequest, GitFileSnapshot};

    struct FakeProvider;

    impl GitProvider for FakeProvider {
        fn read_markdown_file(&self, _request: &GitFileRequest) -> Result<GitFileSnapshot, GitProviderError> {
            Ok(GitFileSnapshot {
                resolved_commit: "a".repeat(40),
                manifest_hash: "sha256:manifest".into(),
                content: "# Hello\r\n".into(),
                content_hash: "unused".into(),
                provider_version: "fake".into(),
            })
        }
    }

    #[test]
    fn reconciles_and_deduplicates_by_content_hash() {
        let controller = MarkdownDocumentController::new(FakeProvider);
        let first = controller
            .reconcile(&MarkdownDocumentRequest {
                repository_url: "https://example.com/repository.git".into(),
                ref_kind: SpecificationRefKind::Branch,
                ref_name: "main".into(),
                path: "docs/spec.md".into(),
                current_content_hash: None,
            })
            .unwrap();
        assert!(first.changed);
        assert_eq!(first.content, "# Hello\n");
        let second = controller
            .reconcile(&MarkdownDocumentRequest {
                current_content_hash: Some(first.content_hash.clone()),
                ..MarkdownDocumentRequest {
                    repository_url: "https://example.com/repository.git".into(),
                    ref_kind: SpecificationRefKind::Branch,
                    ref_name: "main".into(),
                    path: "docs/spec.md".into(),
                    current_content_hash: None,
                }
            })
            .unwrap();
        assert!(!second.changed);
    }
}
