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
