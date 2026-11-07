//! Stable domain data contracts.
//!
//! Invariants and lifecycle operations belong to `specx-core`; these structs
//! only describe facts that cross crate boundaries.

/// Stable identifier used by all specX resources.
///
/// IDs are opaque to consumers. Authorization always combines an ID with its
/// tenant and workspace scope instead of treating it as proof of access.
pub type Id = String;

/// Trusted identity and resource scope for one application operation.
///
/// Hosts derive `tenant_id` and `actor_id` from their authenticated session;
/// the selected workspace must still be checked for active membership.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceContext {
    /// Tenant boundary that owns the workspace.
    pub tenant_id: Id,
    /// Workspace selected for this operation.
    pub workspace_id: Id,
    /// Authenticated actor whose permissions are evaluated.
    pub actor_id: Id,
}

/// Named collaboration space inside a tenant.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    /// Stable workspace identifier.
    pub id: Id,
    /// Tenant that owns this workspace.
    pub tenant_id: Id,
    /// Human-readable name shown in the workspace switcher.
    pub name: String,
    /// Human-readable stable reference used in URLs and exports.
    pub slug: String,
    /// RFC 3339 creation timestamp.
    pub created_at: String,
}

/// Node in the workspace's nested service directory.
///
/// A missing parent represents a top-level directory. `position` is ordered
/// among siblings, so moving a node can change both fields atomically.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceGroup {
    /// Stable directory identifier.
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    /// Parent directory, or `None` for a top-level node.
    pub parent_id: Option<Id>,
    /// Display name; it is not used as an identifier.
    pub name: String,
    /// Zero-based sibling order. Older stored rows default to zero.
    #[serde(default)]
    pub position: i64,
}

/// One member of an atomic directory reordering request.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceGroupOrder {
    /// Directory being moved.
    pub id: Id,
    /// New parent directory, or `None` to move to the top level.
    pub parent_id: Option<Id>,
    /// New position among the destination siblings.
    pub position: i64,
}

/// Workspace label that can be attached to multiple services.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    /// Label shown to users and used for workspace-local lookup.
    pub name: String,
    /// CSS color value used by browser and export clients.
    pub color: String,
}

/// Service entry in a workspace catalog.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    /// Directory that contains the service; missing means ungrouped.
    pub group_id: Option<Id>,
    /// Human-readable service name.
    pub name: String,
    /// Stable human-readable reference unique within the workspace.
    pub slug: String,
    /// Short description shown in catalog and overview views.
    pub description: String,
    /// IDs of workspace tags attached to this service.
    pub tag_ids: Vec<Id>,
    /// RFC 3339 timestamp of the last catalog update.
    pub updated_at: String,
    /// Archived services remain readable but are excluded from normal lists.
    #[serde(default)]
    pub archived: bool,
}

/// Reusable location from which code can be resolved for one or more services.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeSource {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    /// Structural source type. Provider names remain extensible strings.
    pub source_type: CodeSourceType,
    pub provider: String,
    pub display_name: String,
    /// Stable external identity used for workspace-local de-duplication.
    pub canonical_key: String,
    /// Repository URL or host-local root descriptor. It is never a credential.
    pub root_locator: String,
    pub execution_scope: CodeExecutionScope,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub archived: bool,
}

/// Existing Git repository registered in a workspace for reuse by services.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub provider: String,
    pub display_name: String,
    pub canonical_url: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub archived: bool,
}

/// A service's optional link to its single code repository.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRepository {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub service_id: Id,
    pub repository_id: Id,
    pub created_at: String,
    pub updated_at: String,
}

/// A branch, tag, commit, or repository-default ref followed by a service.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRefTrack {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub service_repository_id: Id,
    pub ref_kind: ServiceRefKind,
    pub ref_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A specification's source revision inside its service repository.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecificationCodeBinding {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub specification_id: Id,
    pub service_repository_id: Id,
    /// POSIX path to the Markdown document relative to the repository root.
    pub path: String,
    pub ref_kind: SpecificationRefKind,
    pub ref_name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A ref selector configured for one service-repository relationship.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceRefKind {
    DefaultBranch,
    Branch,
    Tag,
    Commit,
}

/// Git revision selector owned by one specification binding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecificationRefKind {
    Branch,
    Tag,
    Commit,
}

/// Structural source types supported by the first binding workflow.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeSourceType {
    Git,
    LocalFilesystem,
}

/// Host boundary used to resolve a source.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeExecutionScope {
    Server,
    DesktopHost,
}

/// The concrete entity selected inside a source root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeEntityKind {
    RepositoryRoot,
    Directory,
    File,
    Pattern,
}

/// The relationship's importance for a service.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeBindingRole {
    Primary,
    Additional,
}

/// How a binding selects a source revision.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeRevisionKind {
    DefaultBranch,
    Branch,
    Tag,
    Commit,
    WorkingTree,
}

/// A service's explicit relationship with one source entity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCodeBinding {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub service_id: Id,
    pub source_id: Id,
    pub role: CodeBindingRole,
    pub entity_kind: CodeEntityKind,
    /// POSIX path relative to the source root; repository root is empty.
    pub path: String,
    pub revision_kind: CodeRevisionKind,
    pub revision: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Versioned document attached to a service.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Specification {
    pub id: Id,
    pub tenant_id: Id,
    pub workspace_id: Id,
    /// Service that owns this document.
    pub service_id: Id,
    /// Human-readable document title.
    pub title: String,
    /// Content format. New formats require an explicit enum variant.
    pub kind: SpecificationKind,
    /// Current document body. Markdown line endings are normalized by Core.
    pub content: String,
    /// Monotonically increasing revision number.
    pub version: i64,
    /// Content digest populated by persistence when the document is stored.
    pub content_hash: String,
    /// RFC 3339 timestamp of the current revision.
    pub updated_at: String,
}

/// Supported specification body formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecificationKind {
    Markdown,
}
