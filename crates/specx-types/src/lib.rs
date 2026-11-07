//! Shared contracts used by the specX domain, API, and host layers.
//!
//! This crate deliberately contains data and serialization shape only. It has
//! no database, HTTP, UI, or application-state dependencies, so it can be used
//! safely by future CLI, import, and integration clients.

mod models;
mod resource;

pub use models::{
    CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSource, CodeSourceType, Id, Repository,
    Service, ServiceCodeBinding, ServiceGroup, ServiceGroupOrder, ServiceRefKind, ServiceRefTrack, ServiceRepository,
    Specification, SpecificationCodeBinding, SpecificationKind, SpecificationRefKind, Tag, Workspace, WorkspaceContext,
};
pub use resource::{
    ConditionStatus, ContentRef, ResourceCondition, ResourceError, ResourceEvent, ResourceEventKind, ResourcePhase,
    ResourceRef, ResourceStatus,
};
