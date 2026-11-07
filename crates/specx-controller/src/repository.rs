use crate::{GitProviderError, GitRepositoryProvider, GitRepositoryRequest, GitRepositorySnapshot, RepositoryFile};
use specx_types::SpecificationRefKind;

/// Input owned by the generic repository controller. It deliberately contains
/// no specification or service fields, so the controller can be reused by any
/// downstream consumer of repository snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryReconcileRequest {
    pub repository_url: String,
    pub ref_kind: SpecificationRefKind,
    pub ref_name: String,
    pub credential: Option<crate::GitCredential>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySnapshotResult {
    pub resolved_commit: String,
    pub manifest_hash: String,
    pub provider_version: String,
    pub files: Vec<RepositoryFile>,
}

/// Generic repository controller. It resolves one ref, discovers the complete
/// file tree, and returns immutable file evidence. Persistence and downstream
/// interpretation belong to the host and document controllers.
pub struct RepositoryController<P> {
    provider: P,
}

impl<P> RepositoryController<P>
where
    P: GitRepositoryProvider,
{
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn reconcile(
        &self,
        request: &RepositoryReconcileRequest,
    ) -> Result<RepositorySnapshotResult, GitProviderError> {
        let snapshot: GitRepositorySnapshot = self.provider.fetch_repository(&GitRepositoryRequest {
            repository_url: request.repository_url.clone(),
            ref_kind: request.ref_kind,
            ref_name: request.ref_name.clone(),
            credential: request.credential.clone(),
        })?;
        Ok(RepositorySnapshotResult {
            resolved_commit: snapshot.resolved_commit,
            manifest_hash: snapshot.manifest_hash,
            provider_version: snapshot.provider_version,
            files: snapshot.files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GitCredential, GitRepositoryProvider};

    struct FakeProvider;

    impl GitRepositoryProvider for FakeProvider {
        fn fetch_repository(&self, request: &GitRepositoryRequest) -> Result<GitRepositorySnapshot, GitProviderError> {
            assert_eq!(request.ref_name, "main");
            assert_eq!(request.credential, Some(GitCredential { username: None, bearer_token: Some("token".into()) }));
            Ok(GitRepositorySnapshot {
                resolved_commit: "a".repeat(40),
                manifest_hash: "sha256:manifest".into(),
                provider_version: "fake".into(),
                files: vec![RepositoryFile {
                    path: "docs/api.md".into(),
                    content: b"# API\n".to_vec(),
                    content_hash: "sha256:content".into(),
                }],
            })
        }
    }

    #[test]
    fn resolves_a_generic_snapshot_without_domain_knowledge() {
        let controller = RepositoryController::new(FakeProvider);
        let result = controller
            .reconcile(&RepositoryReconcileRequest {
                repository_url: "https://example.com/repository.git".into(),
                ref_kind: SpecificationRefKind::Branch,
                ref_name: "main".into(),
                credential: Some(GitCredential { username: None, bearer_token: Some("token".into()) }),
            })
            .unwrap();
        assert_eq!(result.files[0].path, "docs/api.md");
        assert_eq!(result.resolved_commit.len(), 40);
    }
}
