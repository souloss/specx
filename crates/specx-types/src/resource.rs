use crate::Id;

/// Stable identity of a resource within its tenant and workspace scope.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRef {
    pub tenant_id: Id,
    pub workspace_id: Id,
    pub kind: String,
    pub id: Id,
}

impl ResourceRef {
    pub fn new(tenant_id: Id, workspace_id: Id, kind: impl Into<String>, id: Id) -> Self {
        Self { tenant_id, workspace_id, kind: kind.into(), id }
    }
}

/// A reference to bytes managed by the platform ContentStore.
///
/// `store_kind` is an implementation detail and is not a user-selectable
/// resource field. Domain resources should persist this reference together
/// with its digest and lifecycle metadata.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRef {
    pub store_kind: String,
    pub object_key: String,
    pub media_type: String,
    pub byte_length: u64,
    pub content_hash: String,
    pub created_at: String,
    pub retention: Option<String>,
}

/// High-level state exposed by a controller-managed resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePhase {
    Pending,
    Running,
    Ready,
    Degraded,
    Stalled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConditionStatus {
    True,
    False,
    Unknown,
}

/// A stable, machine-readable condition produced by a controller.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCondition {
    #[serde(rename = "type")]
    pub type_name: String,
    pub status: ConditionStatus,
    pub reason: String,
    pub message_key: String,
    pub observed_generation: u64,
    pub observed_at: String,
}

/// Error identity suitable for status and retry decisions.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceError {
    pub code: String,
    pub retryable: bool,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatus {
    pub observed_generation: u64,
    pub phase: ResourcePhase,
    pub conditions: Vec<ResourceCondition>,
    pub last_error: Option<ResourceError>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceEventKind {
    Created,
    Updated,
    Deleted,
    ReconcileRequested,
    StatusChanged,
    SnapshotReady,
}

/// Durable event payload used only to wake controllers. The resource and
/// status remain the source of truth when an event is duplicated or lost.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEvent {
    pub event_id: Id,
    pub resource: ResourceRef,
    pub generation: u64,
    pub kind: ResourceEventKind,
    pub deduplication_key: String,
    pub occurred_at: String,
}
