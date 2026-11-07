//! Generic controller runtime primitives.
//!
//! Controllers observe resource events, reconcile against the current resource
//! generation, and return an explicit completion, retry, or failure result.
//! This crate deliberately contains no HTTP, database, or domain-specific
//! controller implementation.

mod content_store;
mod git_provider;
mod markdown;
mod repository;
mod runtime;

pub use content_store::{ContentStore, ContentStoreError, FileContentStore};
pub use git_provider::{
    CommandGitProvider, GitCredential, GitFileRequest, GitFileSnapshot, GitProvider, GitProviderError,
    GitRepositoryProvider, GitRepositoryRequest, GitRepositorySnapshot, RepositoryFile, validate_markdown_path,
};
pub use markdown::{MarkdownDocumentController, MarkdownDocumentRequest, MarkdownDocumentResult};
pub use repository::{RepositoryController, RepositoryReconcileRequest, RepositorySnapshotResult};
pub use runtime::{
    Controller, ControllerDescriptor, ControllerError, ControllerEventRouter, ControllerRuntime, ReconcileRequest,
    ReconcileResult, RunOutcome, RuntimeError,
};
