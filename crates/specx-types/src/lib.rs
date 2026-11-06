//! Shared contracts used by the specX domain, API, and host layers.
//!
//! This crate deliberately contains data and serialization shape only. It has
//! no database, HTTP, UI, or application-state dependencies, so it can be used
//! safely by future CLI, import, and integration clients.

mod models;

pub use models::{
    CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSource, CodeSourceType, Id, Service,
    ServiceCodeBinding, ServiceGroup, ServiceGroupOrder, Specification, SpecificationKind, Tag, Workspace,
    WorkspaceContext,
};
