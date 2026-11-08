//! Service-centered domain model for specX.
//!
//! This crate owns workspace facts and their invariants. UI session state such as
//! open tabs and filters belongs to the host application.

use std::collections::{HashMap, HashSet};

pub use specx_types::{
    CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSource, CodeSourceType, Id, Repository,
    RepositorySnapshot, Service, ServiceCodeBinding, ServiceGroup, ServiceGroupOrder, ServiceRefKind, ServiceRefTrack,
    ServiceRepository, Specification, SpecificationCodeBinding, SpecificationKind, SpecificationRefKind, Tag,
    Workspace, WorkspaceContext,
};

pub mod persistence;
#[cfg(feature = "postgres")]
pub mod postgres;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    EmptyName,
    EmptySlug,
    EmptyContent,
    NotFound(&'static str),
    ScopeMismatch,
    InvalidParent,
    GroupCycle,
    InvalidGroupOrder,
    DuplicateTag,
    DuplicateSlug,
    DuplicateCodeSource,
    DuplicateCodeBinding,
    InvalidCodeSource,
    InvalidCodeBinding,
    DuplicateRepository,
    InvalidRepository,
    ServiceRepositoryExists,
    DuplicateRefTrack,
    InvalidRefTrack,
    InvalidSpecificationCodeBinding,
    InvalidRepositorySnapshot,
    ManagedSpecification,
    HasDependents(&'static str),
}

#[derive(Clone, Default)]
pub struct Core {
    groups: HashMap<Id, ServiceGroup>,
    tags: HashMap<Id, Tag>,
    services: HashMap<Id, Service>,
    code_sources: HashMap<Id, CodeSource>,
    code_bindings: HashMap<Id, ServiceCodeBinding>,
    repositories: HashMap<Id, Repository>,
    repository_snapshots: HashMap<Id, RepositorySnapshot>,
    service_repositories: HashMap<Id, ServiceRepository>,
    ref_tracks: HashMap<Id, ServiceRefTrack>,
    specification_code_bindings: HashMap<Id, SpecificationCodeBinding>,
    specifications: HashMap<Id, Specification>,
}

impl Core {
    #[cfg(feature = "postgres")]
    pub(crate) fn groups(&self) -> impl Iterator<Item = &ServiceGroup> {
        self.groups.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn tags(&self) -> impl Iterator<Item = &Tag> {
        self.tags.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn services(&self) -> impl Iterator<Item = &Service> {
        self.services.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn code_sources(&self) -> impl Iterator<Item = &CodeSource> {
        self.code_sources.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn code_bindings(&self) -> impl Iterator<Item = &ServiceCodeBinding> {
        self.code_bindings.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn repositories(&self) -> impl Iterator<Item = &Repository> {
        self.repositories.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn service_repositories(&self) -> impl Iterator<Item = &ServiceRepository> {
        self.service_repositories.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn repository_snapshots(&self) -> impl Iterator<Item = &RepositorySnapshot> {
        self.repository_snapshots.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn ref_tracks(&self) -> impl Iterator<Item = &ServiceRefTrack> {
        self.ref_tracks.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn specification_code_bindings(&self) -> impl Iterator<Item = &SpecificationCodeBinding> {
        self.specification_code_bindings.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn specifications(&self) -> impl Iterator<Item = &Specification> {
        self.specifications.values()
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_group(&mut self, group: ServiceGroup) {
        self.groups.insert(group.id.clone(), group);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_tag(&mut self, tag: Tag) {
        self.tags.insert(tag.id.clone(), tag);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_service(&mut self, service: Service) {
        self.services.insert(service.id.clone(), service);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_code_source(&mut self, source: CodeSource) {
        self.code_sources.insert(source.id.clone(), source);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_code_binding(&mut self, binding: ServiceCodeBinding) {
        self.code_bindings.insert(binding.id.clone(), binding);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_repository(&mut self, repository: Repository) {
        self.repositories.insert(repository.id.clone(), repository);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_service_repository(&mut self, relation: ServiceRepository) {
        self.service_repositories.insert(relation.id.clone(), relation);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_repository_snapshot(&mut self, snapshot: RepositorySnapshot) {
        self.repository_snapshots.insert(snapshot.id.clone(), snapshot);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_ref_track(&mut self, track: ServiceRefTrack) {
        self.ref_tracks.insert(track.id.clone(), track);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_specification_code_binding(&mut self, binding: SpecificationCodeBinding) {
        self.specification_code_bindings.insert(binding.id.clone(), binding);
    }

    #[cfg(feature = "postgres")]
    pub(crate) fn insert_specification(&mut self, specification: Specification) {
        self.specifications.insert(specification.id.clone(), specification);
    }

    pub fn create_group(
        &mut self,
        context: &WorkspaceContext,
        id: Id,
        name: String,
        parent_id: Option<Id>,
    ) -> Result<ServiceGroup, CoreError> {
        let position = self
            .list_groups(context)
            .into_iter()
            .filter(|group| group.parent_id == parent_id)
            .map(|group| group.position)
            .max()
            .unwrap_or(-1)
            .saturating_add(1)
            .max(0);
        self.create_group_with_position(context, id, name, parent_id, position)
    }

    pub fn create_group_with_position(
        &mut self,
        context: &WorkspaceContext,
        id: Id,
        name: String,
        parent_id: Option<Id>,
        position: i64,
    ) -> Result<ServiceGroup, CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        if let Some(parent_id) = &parent_id {
            let parent = self.groups.get(parent_id).ok_or(CoreError::InvalidParent)?;
            ensure_tenant(context, &parent.tenant_id)?;
            ensure_workspace(context, &parent.workspace_id)?;
        }
        let group = ServiceGroup {
            id: id.clone(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            parent_id,
            name,
            position,
        };
        self.groups.insert(id, group.clone());
        Ok(group)
    }

    pub fn move_group(
        &mut self,
        context: &WorkspaceContext,
        group_id: &str,
        parent_id: Option<Id>,
    ) -> Result<(), CoreError> {
        let group = self.groups.get(group_id).ok_or(CoreError::NotFound("group"))?;
        ensure_tenant(context, &group.tenant_id)?;
        ensure_workspace(context, &group.workspace_id)?;
        if let Some(parent_id) = &parent_id {
            let parent = self.groups.get(parent_id).ok_or(CoreError::InvalidParent)?;
            ensure_tenant(context, &parent.tenant_id)?;
            ensure_workspace(context, &parent.workspace_id)?;
            if parent.id == group_id || self.is_descendant(parent_id, group_id) {
                return Err(CoreError::GroupCycle);
            }
        }
        self.groups.get_mut(group_id).expect("group was checked").parent_id = parent_id;
        Ok(())
    }

    pub fn update_group(
        &mut self,
        context: &WorkspaceContext,
        group_id: &str,
        name: String,
        parent_id: Option<Id>,
    ) -> Result<ServiceGroup, CoreError> {
        let group = self.groups.get(group_id).ok_or(CoreError::NotFound("group"))?;
        let position = if group.parent_id == parent_id {
            group.position
        } else {
            self.list_groups(context)
                .into_iter()
                .filter(|sibling| sibling.parent_id == parent_id)
                .map(|sibling| sibling.position)
                .max()
                .unwrap_or(-1)
                .saturating_add(1)
                .max(0)
        };
        self.update_group_with_position(context, group_id, name, parent_id, position)
    }

    pub fn update_group_with_position(
        &mut self,
        context: &WorkspaceContext,
        group_id: &str,
        name: String,
        parent_id: Option<Id>,
        position: i64,
    ) -> Result<ServiceGroup, CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        let group = self.groups.get(group_id).ok_or(CoreError::NotFound("group"))?;
        ensure_tenant(context, &group.tenant_id)?;
        ensure_workspace(context, &group.workspace_id)?;
        self.move_group(context, group_id, parent_id.clone())?;
        let group = self.groups.get_mut(group_id).expect("group was checked");
        group.name = name;
        group.parent_id = parent_id;
        group.position = position;
        Ok(group.clone())
    }

    pub fn reorder_groups(&mut self, context: &WorkspaceContext, items: &[ServiceGroupOrder]) -> Result<(), CoreError> {
        let scoped_groups = self.list_groups(context);
        if scoped_groups.len() != items.len() {
            return Err(CoreError::InvalidGroupOrder);
        }

        let mut order_by_id = HashMap::with_capacity(items.len());
        let mut positions = HashSet::with_capacity(items.len());
        for item in items {
            if item.position < 0 || order_by_id.insert(item.id.as_str(), item).is_some() {
                return Err(CoreError::InvalidGroupOrder);
            }
            if !positions.insert((item.parent_id.as_deref(), item.position)) {
                return Err(CoreError::InvalidGroupOrder);
            }
            let group = self.groups.get(&item.id).ok_or(CoreError::NotFound("group"))?;
            ensure_tenant(context, &group.tenant_id)?;
            ensure_workspace(context, &group.workspace_id)?;
        }
        for item in items {
            if let Some(parent_id) = item.parent_id.as_deref()
                && !order_by_id.contains_key(parent_id)
            {
                return Err(CoreError::InvalidParent);
            }
            let mut path = HashSet::new();
            let mut cursor = Some(item.id.as_str());
            while let Some(id) = cursor {
                if !path.insert(id) {
                    return Err(CoreError::GroupCycle);
                }
                cursor = order_by_id.get(id).and_then(|parent| parent.parent_id.as_deref());
            }
        }

        for item in items {
            let group = self.groups.get_mut(&item.id).expect("group order was validated");
            group.parent_id.clone_from(&item.parent_id);
            group.position = item.position;
        }
        Ok(())
    }

    pub fn delete_group(&mut self, context: &WorkspaceContext, group_id: &str) -> Result<ServiceGroup, CoreError> {
        let group = self.groups.get(group_id).cloned().ok_or(CoreError::NotFound("group"))?;
        ensure_tenant(context, &group.tenant_id)?;
        ensure_workspace(context, &group.workspace_id)?;
        if self.groups.values().any(|child| child.parent_id.as_deref() == Some(group_id)) {
            return Err(CoreError::HasDependents("group"));
        }
        if self.services.values().any(|service| service.group_id.as_deref() == Some(group_id)) {
            return Err(CoreError::HasDependents("service"));
        }
        self.groups.remove(group_id);
        Ok(group)
    }

    pub fn create_tag(
        &mut self,
        context: &WorkspaceContext,
        id: Id,
        name: String,
        color: String,
    ) -> Result<Tag, CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        let tag = Tag {
            id: id.clone(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            name,
            color,
        };
        self.tags.insert(id, tag.clone());
        Ok(tag)
    }

    pub fn update_tag(
        &mut self,
        context: &WorkspaceContext,
        tag_id: &str,
        name: String,
        color: String,
    ) -> Result<Tag, CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        let tag = self.tags.get_mut(tag_id).ok_or(CoreError::NotFound("tag"))?;
        ensure_tenant(context, &tag.tenant_id)?;
        ensure_workspace(context, &tag.workspace_id)?;
        tag.name = name;
        tag.color = color;
        Ok(tag.clone())
    }

    pub fn delete_tag(&mut self, context: &WorkspaceContext, tag_id: &str) -> Result<Tag, CoreError> {
        let tag = self.tags.get(tag_id).cloned().ok_or(CoreError::NotFound("tag"))?;
        ensure_tenant(context, &tag.tenant_id)?;
        ensure_workspace(context, &tag.workspace_id)?;
        if self.services.values().any(|service| service.tag_ids.iter().any(|id| id == tag_id)) {
            return Err(CoreError::HasDependents("service"));
        }
        self.tags.remove(tag_id);
        Ok(tag)
    }

    pub fn create_service(&mut self, context: &WorkspaceContext, service: Service) -> Result<Service, CoreError> {
        self.validate_service(context, &service, None)?;
        self.services.insert(service.id.clone(), service.clone());
        Ok(service)
    }

    pub fn update_service(
        &mut self,
        context: &WorkspaceContext,
        service_id: &str,
        name: String,
        slug: String,
        description: String,
        group_id: Option<Id>,
        tag_ids: Vec<Id>,
    ) -> Result<Service, CoreError> {
        let mut service = self.services.get(service_id).cloned().ok_or(CoreError::NotFound("service"))?;
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        service.name = name;
        service.slug = slug;
        service.description = description;
        service.group_id = group_id;
        service.tag_ids = tag_ids;
        service.updated_at = chrono::Utc::now().to_rfc3339();
        self.validate_service(context, &service, Some(service_id))?;
        self.services.insert(service.id.clone(), service.clone());
        Ok(service)
    }

    pub fn set_service_archived(
        &mut self,
        context: &WorkspaceContext,
        service_id: &str,
        archived: bool,
    ) -> Result<Service, CoreError> {
        let service = self.services.get_mut(service_id).ok_or(CoreError::NotFound("service"))?;
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        service.archived = archived;
        service.updated_at = chrono::Utc::now().to_rfc3339();
        Ok(service.clone())
    }

    pub fn delete_service(&mut self, context: &WorkspaceContext, service_id: &str) -> Result<Service, CoreError> {
        let service = self.services.get(service_id).cloned().ok_or(CoreError::NotFound("service"))?;
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        self.services.remove(service_id);
        self.code_bindings.retain(|_, binding| binding.service_id != service_id);
        let relation_ids: HashSet<_> = self
            .service_repositories
            .values()
            .filter(|relation| relation.service_id == service_id)
            .map(|relation| relation.id.clone())
            .collect();
        self.ref_tracks.retain(|_, track| !relation_ids.contains(&track.service_repository_id));
        self.service_repositories.retain(|_, relation| relation.service_id != service_id);
        let specification_ids: HashSet<_> = self
            .specifications
            .values()
            .filter(|specification| specification.service_id == service_id)
            .map(|specification| specification.id.clone())
            .collect();
        self.specification_code_bindings.retain(|_, binding| !specification_ids.contains(&binding.specification_id));
        self.specifications.retain(|_, specification| specification.service_id != service_id);
        Ok(service)
    }

    pub fn create_repository(
        &mut self,
        context: &WorkspaceContext,
        id: Id,
        raw_url: &str,
    ) -> Result<Repository, CoreError> {
        let (canonical_url, provider, display_name) = normalize_repository_url(raw_url)?;
        let repository = Repository {
            id,
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            provider,
            display_name,
            canonical_url,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            archived: false,
        };
        self.validate_repository(context, &repository, None)?;
        self.repositories.insert(repository.id.clone(), repository.clone());
        Ok(repository)
    }

    pub fn list_repositories(&self, context: &WorkspaceContext) -> Vec<&Repository> {
        self.repositories
            .values()
            .filter(|repository| {
                repository.tenant_id == context.tenant_id && repository.workspace_id == context.workspace_id
            })
            .collect()
    }

    pub fn repository_by_canonical_url(&self, context: &WorkspaceContext, canonical_url: &str) -> Option<&Repository> {
        self.list_repositories(context).into_iter().find(|repository| repository.canonical_url == canonical_url)
    }

    pub fn link_service_repository(
        &mut self,
        context: &WorkspaceContext,
        id: Id,
        service_id: &str,
        repository_id: &str,
    ) -> Result<ServiceRepository, CoreError> {
        let service = self.services.get(service_id).ok_or(CoreError::NotFound("service"))?;
        let repository = self.repositories.get(repository_id).ok_or(CoreError::NotFound("repository"))?;
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        ensure_tenant(context, &repository.tenant_id)?;
        ensure_workspace(context, &repository.workspace_id)?;
        if let Some(existing) = self.service_repositories.values().find(|relation| {
            relation.tenant_id == context.tenant_id
                && relation.workspace_id == context.workspace_id
                && relation.service_id == service_id
        }) {
            if existing.repository_id == repository_id {
                return Ok(existing.clone());
            }
            return Err(CoreError::ServiceRepositoryExists);
        }
        let now = chrono::Utc::now().to_rfc3339();
        let relation = ServiceRepository {
            id,
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id: service_id.to_owned(),
            repository_id: repository_id.to_owned(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.service_repositories.insert(relation.id.clone(), relation.clone());
        Ok(relation)
    }

    pub fn service_repository(&self, context: &WorkspaceContext, service_id: &str) -> Option<&ServiceRepository> {
        self.service_repositories.values().find(|relation| {
            relation.tenant_id == context.tenant_id
                && relation.workspace_id == context.workspace_id
                && relation.service_id == service_id
        })
    }

    pub fn repository(&self, context: &WorkspaceContext, repository_id: &str) -> Option<&Repository> {
        self.repositories.get(repository_id).filter(|repository| {
            repository.tenant_id == context.tenant_id && repository.workspace_id == context.workspace_id
        })
    }

    /// Returns snapshots for one repository in newest-first order.
    pub fn list_repository_snapshots(
        &self,
        context: &WorkspaceContext,
        repository_id: &str,
    ) -> Vec<&RepositorySnapshot> {
        let mut snapshots: Vec<_> = self
            .repository_snapshots
            .values()
            .filter(|snapshot| {
                snapshot.tenant_id == context.tenant_id
                    && snapshot.workspace_id == context.workspace_id
                    && snapshot.repository_id == repository_id
            })
            .collect();
        snapshots.sort_by(|left, right| {
            right
                .fetched_at
                .cmp(&left.fetched_at)
                .then_with(|| right.created_at.cmp(&left.created_at))
                .then_with(|| right.id.cmp(&left.id))
        });
        snapshots
    }

    pub fn repository_snapshot(&self, context: &WorkspaceContext, snapshot_id: &str) -> Option<&RepositorySnapshot> {
        self.repository_snapshots
            .get(snapshot_id)
            .filter(|snapshot| snapshot.tenant_id == context.tenant_id && snapshot.workspace_id == context.workspace_id)
    }

    /// Inserts immutable snapshot evidence, returning the existing row when
    /// the same repository/ref/commit has already been observed.
    pub fn record_repository_snapshot(
        &mut self,
        context: &WorkspaceContext,
        snapshot: RepositorySnapshot,
    ) -> Result<RepositorySnapshot, CoreError> {
        let repository = self.repository(context, &snapshot.repository_id).ok_or(CoreError::NotFound("repository"))?;
        if snapshot.tenant_id != context.tenant_id
            || snapshot.workspace_id != context.workspace_id
            || repository.tenant_id != context.tenant_id
            || repository.workspace_id != context.workspace_id
            || snapshot.source_ref.trim().is_empty()
            || snapshot.resolved_commit.trim().is_empty()
            || snapshot.manifest_hash.trim().is_empty()
            || snapshot.provider_version.trim().is_empty()
            || snapshot.fetched_at.trim().is_empty()
        {
            return Err(CoreError::InvalidRepositorySnapshot);
        }
        if let Some(existing) = self.repository_snapshots.values().find(|existing| {
            existing.tenant_id == context.tenant_id
                && existing.workspace_id == context.workspace_id
                && existing.repository_id == snapshot.repository_id
                && existing.source_ref == snapshot.source_ref
                && existing.resolved_commit == snapshot.resolved_commit
        }) {
            return Ok(existing.clone());
        }
        self.repository_snapshots.insert(snapshot.id.clone(), snapshot.clone());
        Ok(snapshot)
    }

    pub fn list_service_ref_tracks(
        &self,
        context: &WorkspaceContext,
        service_repository_id: &str,
    ) -> Vec<&ServiceRefTrack> {
        self.ref_tracks
            .values()
            .filter(|track| {
                track.tenant_id == context.tenant_id
                    && track.workspace_id == context.workspace_id
                    && track.service_repository_id == service_repository_id
            })
            .collect()
    }

    pub fn service_ref_track(&self, context: &WorkspaceContext, track_id: &str) -> Option<&ServiceRefTrack> {
        self.ref_tracks
            .get(track_id)
            .filter(|track| track.tenant_id == context.tenant_id && track.workspace_id == context.workspace_id)
    }

    pub fn create_service_ref_track(
        &mut self,
        context: &WorkspaceContext,
        mut track: ServiceRefTrack,
    ) -> Result<ServiceRefTrack, CoreError> {
        let relation = self
            .service_repositories
            .get(&track.service_repository_id)
            .ok_or(CoreError::NotFound("service_repository"))?;
        ensure_tenant(context, &track.tenant_id)?;
        ensure_workspace(context, &track.workspace_id)?;
        ensure_tenant(context, &relation.tenant_id)?;
        ensure_workspace(context, &relation.workspace_id)?;
        if let Some(name) = &mut track.ref_name {
            *name = name.trim().to_owned();
            if track.ref_kind == ServiceRefKind::Commit {
                name.make_ascii_lowercase();
            }
        }
        validate_ref_track(&track)?;
        if self.ref_tracks.values().any(|existing| {
            existing.service_repository_id == track.service_repository_id
                && existing.ref_kind == track.ref_kind
                && existing.ref_name == track.ref_name
        }) {
            return Err(CoreError::DuplicateRefTrack);
        }
        if track.ref_kind == ServiceRefKind::DefaultBranch
            && self.ref_tracks.values().any(|existing| {
                existing.service_repository_id == track.service_repository_id
                    && existing.ref_kind == ServiceRefKind::DefaultBranch
            })
        {
            return Err(CoreError::DuplicateRefTrack);
        }
        self.ref_tracks.insert(track.id.clone(), track.clone());
        Ok(track)
    }

    pub fn delete_service_repository(
        &mut self,
        context: &WorkspaceContext,
        service_repository_id: &str,
    ) -> Result<ServiceRepository, CoreError> {
        let relation = self
            .service_repositories
            .get(service_repository_id)
            .cloned()
            .ok_or(CoreError::NotFound("service_repository"))?;
        ensure_tenant(context, &relation.tenant_id)?;
        ensure_workspace(context, &relation.workspace_id)?;
        self.ref_tracks.retain(|_, track| track.service_repository_id != service_repository_id);
        self.specification_code_bindings.retain(|_, binding| binding.service_repository_id != service_repository_id);
        self.service_repositories.remove(service_repository_id);
        Ok(relation)
    }

    pub fn delete_service_ref_track(
        &mut self,
        context: &WorkspaceContext,
        track_id: &str,
    ) -> Result<ServiceRefTrack, CoreError> {
        let track = self.ref_tracks.get(track_id).cloned().ok_or(CoreError::NotFound("ref_track"))?;
        ensure_tenant(context, &track.tenant_id)?;
        ensure_workspace(context, &track.workspace_id)?;
        self.ref_tracks.remove(track_id);
        Ok(track)
    }

    pub fn create_code_source(
        &mut self,
        context: &WorkspaceContext,
        source: CodeSource,
    ) -> Result<CodeSource, CoreError> {
        self.validate_code_source(context, &source, None)?;
        self.code_sources.insert(source.id.clone(), source.clone());
        Ok(source)
    }

    pub fn update_code_source(
        &mut self,
        context: &WorkspaceContext,
        source_id: &str,
        source_type: CodeSourceType,
        display_name: String,
        provider: String,
        canonical_key: String,
        root_locator: String,
        execution_scope: CodeExecutionScope,
    ) -> Result<CodeSource, CoreError> {
        let mut source = self.code_sources.get(source_id).cloned().ok_or(CoreError::NotFound("code_source"))?;
        ensure_tenant(context, &source.tenant_id)?;
        ensure_workspace(context, &source.workspace_id)?;
        source.display_name = display_name;
        source.source_type = source_type;
        source.provider = provider;
        source.canonical_key = canonical_key;
        source.root_locator = root_locator;
        source.execution_scope = execution_scope;
        source.updated_at = chrono::Utc::now().to_rfc3339();
        self.validate_code_source(context, &source, Some(source_id))?;
        self.code_sources.insert(source.id.clone(), source.clone());
        Ok(source)
    }

    pub fn delete_code_source(&mut self, context: &WorkspaceContext, source_id: &str) -> Result<CodeSource, CoreError> {
        let source = self.code_sources.get(source_id).cloned().ok_or(CoreError::NotFound("code_source"))?;
        ensure_tenant(context, &source.tenant_id)?;
        ensure_workspace(context, &source.workspace_id)?;
        if self.code_bindings.values().any(|binding| binding.source_id == source_id) {
            return Err(CoreError::HasDependents("code_binding"));
        }
        self.code_sources.remove(source_id);
        Ok(source)
    }

    pub fn create_code_binding(
        &mut self,
        context: &WorkspaceContext,
        binding: ServiceCodeBinding,
    ) -> Result<ServiceCodeBinding, CoreError> {
        self.validate_code_binding(context, &binding, None)?;
        self.code_bindings.insert(binding.id.clone(), binding.clone());
        Ok(binding)
    }

    pub fn update_code_binding(
        &mut self,
        context: &WorkspaceContext,
        binding_id: &str,
        role: CodeBindingRole,
        entity_kind: CodeEntityKind,
        path: String,
        revision_kind: CodeRevisionKind,
        revision: Option<String>,
    ) -> Result<ServiceCodeBinding, CoreError> {
        let mut binding = self.code_bindings.get(binding_id).cloned().ok_or(CoreError::NotFound("code_binding"))?;
        ensure_tenant(context, &binding.tenant_id)?;
        ensure_workspace(context, &binding.workspace_id)?;
        binding.role = role;
        binding.entity_kind = entity_kind;
        binding.path = path;
        binding.revision_kind = revision_kind;
        binding.revision = revision;
        binding.updated_at = chrono::Utc::now().to_rfc3339();
        self.validate_code_binding(context, &binding, Some(binding_id))?;
        self.code_bindings.insert(binding.id.clone(), binding.clone());
        Ok(binding)
    }

    pub fn delete_code_binding(
        &mut self,
        context: &WorkspaceContext,
        binding_id: &str,
    ) -> Result<ServiceCodeBinding, CoreError> {
        let binding = self.code_bindings.get(binding_id).cloned().ok_or(CoreError::NotFound("code_binding"))?;
        ensure_tenant(context, &binding.tenant_id)?;
        ensure_workspace(context, &binding.workspace_id)?;
        self.code_bindings.remove(binding_id);
        Ok(binding)
    }

    fn validate_service(
        &self,
        context: &WorkspaceContext,
        service: &Service,
        current_id: Option<&str>,
    ) -> Result<(), CoreError> {
        if service.name.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        if service.slug.trim().is_empty() {
            return Err(CoreError::EmptySlug);
        }
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        if self.services.values().any(|existing| {
            Some(existing.id.as_str()) != current_id
                && existing.tenant_id == context.tenant_id
                && existing.workspace_id == context.workspace_id
                && existing.slug == service.slug
        }) {
            return Err(CoreError::DuplicateSlug);
        }
        if let Some(group_id) = &service.group_id {
            let group = self.groups.get(group_id).ok_or(CoreError::NotFound("group"))?;
            ensure_tenant(context, &group.tenant_id)?;
            ensure_workspace(context, &group.workspace_id)?;
        }
        let mut seen = HashSet::new();
        for tag_id in &service.tag_ids {
            if !seen.insert(tag_id) {
                return Err(CoreError::DuplicateTag);
            }
            let tag = self.tags.get(tag_id).ok_or(CoreError::NotFound("tag"))?;
            ensure_tenant(context, &tag.tenant_id)?;
            ensure_workspace(context, &tag.workspace_id)?;
        }
        Ok(())
    }

    pub fn create_markdown_specification(
        &mut self,
        context: &WorkspaceContext,
        mut specification: Specification,
    ) -> Result<Specification, CoreError> {
        if specification.title.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        if specification.content.trim().is_empty() {
            return Err(CoreError::EmptyContent);
        }
        ensure_tenant(context, &specification.tenant_id)?;
        ensure_workspace(context, &specification.workspace_id)?;
        let service = self.services.get(&specification.service_id).ok_or(CoreError::NotFound("service"))?;
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        specification.content = specx_formats::normalize_line_endings(&specification.content);
        self.specifications.insert(specification.id.clone(), specification.clone());
        Ok(specification)
    }

    pub fn update_markdown_specification(
        &mut self,
        context: &WorkspaceContext,
        specification_id: &str,
        title: String,
        content: String,
    ) -> Result<Specification, CoreError> {
        if title.trim().is_empty() {
            return Err(CoreError::EmptyName);
        }
        if content.trim().is_empty() {
            return Err(CoreError::EmptyContent);
        }
        let mut specification =
            self.specifications.get(specification_id).cloned().ok_or(CoreError::NotFound("specification"))?;
        ensure_tenant(context, &specification.tenant_id)?;
        ensure_workspace(context, &specification.workspace_id)?;
        if self.specification_code_binding(context, specification_id).is_some_and(|binding| !binding.path.is_empty()) {
            return Err(CoreError::ManagedSpecification);
        }
        specification.title = title;
        specification.content = specx_formats::normalize_line_endings(&content);
        specification.updated_at = chrono::Utc::now().to_rfc3339();
        self.specifications.insert(specification.id.clone(), specification.clone());
        Ok(specification)
    }

    /// Apply content produced by the controller for a repository-managed
    /// specification. Interactive edits use `update_markdown_specification`,
    /// which intentionally rejects managed sources; controllers are the only
    /// caller allowed to update their observed source content.
    pub fn update_managed_specification_content(
        &mut self,
        context: &WorkspaceContext,
        specification_id: &str,
        content: String,
    ) -> Result<Specification, CoreError> {
        if content.trim().is_empty() {
            return Err(CoreError::EmptyContent);
        }
        let mut specification =
            self.specifications.get(specification_id).cloned().ok_or(CoreError::NotFound("specification"))?;
        ensure_tenant(context, &specification.tenant_id)?;
        ensure_workspace(context, &specification.workspace_id)?;
        let binding = self
            .specification_code_binding(context, specification_id)
            .ok_or(CoreError::InvalidSpecificationCodeBinding)?;
        if binding.path.is_empty() {
            return Err(CoreError::InvalidSpecificationCodeBinding);
        }
        specification.content = specx_formats::normalize_line_endings(&content);
        specification.updated_at = chrono::Utc::now().to_rfc3339();
        self.specifications.insert(specification.id.clone(), specification.clone());
        Ok(specification)
    }

    pub fn delete_specification(
        &mut self,
        context: &WorkspaceContext,
        specification_id: &str,
    ) -> Result<Specification, CoreError> {
        let specification =
            self.specifications.get(specification_id).cloned().ok_or(CoreError::NotFound("specification"))?;
        ensure_tenant(context, &specification.tenant_id)?;
        ensure_workspace(context, &specification.workspace_id)?;
        if self.specification_code_binding(context, specification_id).is_some_and(|binding| !binding.path.is_empty()) {
            return Err(CoreError::ManagedSpecification);
        }
        self.specifications.remove(specification_id);
        self.specification_code_bindings.retain(|_, binding| binding.specification_id != specification_id);
        Ok(specification)
    }

    pub fn specification_code_binding(
        &self,
        context: &WorkspaceContext,
        specification_id: &str,
    ) -> Option<&SpecificationCodeBinding> {
        self.specification_code_bindings.values().find(|binding| {
            binding.tenant_id == context.tenant_id
                && binding.workspace_id == context.workspace_id
                && binding.specification_id == specification_id
        })
    }

    pub fn set_specification_code_binding(
        &mut self,
        context: &WorkspaceContext,
        mut binding: SpecificationCodeBinding,
    ) -> Result<SpecificationCodeBinding, CoreError> {
        let specification =
            self.specifications.get(&binding.specification_id).ok_or(CoreError::NotFound("specification"))?;
        let relation = self
            .service_repositories
            .get(&binding.service_repository_id)
            .ok_or(CoreError::NotFound("service_repository"))?;
        ensure_tenant(context, &binding.tenant_id)?;
        ensure_workspace(context, &binding.workspace_id)?;
        ensure_tenant(context, &specification.tenant_id)?;
        ensure_workspace(context, &specification.workspace_id)?;
        ensure_tenant(context, &relation.tenant_id)?;
        ensure_workspace(context, &relation.workspace_id)?;
        if relation.service_id != specification.service_id {
            return Err(CoreError::InvalidSpecificationCodeBinding);
        }
        binding.path = binding.path.trim().to_owned();
        binding.ref_name = binding.ref_name.trim().to_owned();
        if binding.ref_kind == SpecificationRefKind::Commit {
            binding.ref_name.make_ascii_lowercase();
        }
        validate_specification_ref(&binding)?;
        if let Some(existing) = self.specification_code_binding(context, &binding.specification_id) {
            if !existing.path.is_empty() {
                return Err(CoreError::ManagedSpecification);
            }
            if existing.id != binding.id {
                return Err(CoreError::InvalidSpecificationCodeBinding);
            }
        }
        self.specification_code_bindings.insert(binding.id.clone(), binding.clone());
        Ok(binding)
    }

    pub fn delete_specification_code_binding(
        &mut self,
        context: &WorkspaceContext,
        specification_id: &str,
    ) -> Result<SpecificationCodeBinding, CoreError> {
        let binding = self
            .specification_code_binding(context, specification_id)
            .cloned()
            .ok_or(CoreError::NotFound("specification_code_binding"))?;
        ensure_tenant(context, &binding.tenant_id)?;
        ensure_workspace(context, &binding.workspace_id)?;
        self.specification_code_bindings.remove(&binding.id);
        Ok(binding)
    }

    pub fn list_services(&self, context: &WorkspaceContext) -> Vec<&Service> {
        self.services
            .values()
            .filter(|service| service.tenant_id == context.tenant_id && service.workspace_id == context.workspace_id)
            .collect()
    }

    pub fn list_code_sources(&self, context: &WorkspaceContext) -> Vec<&CodeSource> {
        self.code_sources
            .values()
            .filter(|source| source.tenant_id == context.tenant_id && source.workspace_id == context.workspace_id)
            .collect()
    }

    pub fn list_code_bindings(&self, context: &WorkspaceContext) -> Vec<&ServiceCodeBinding> {
        self.code_bindings
            .values()
            .filter(|binding| binding.tenant_id == context.tenant_id && binding.workspace_id == context.workspace_id)
            .collect()
    }

    pub fn list_service_code_bindings(&self, context: &WorkspaceContext, service_id: &str) -> Vec<&ServiceCodeBinding> {
        self.list_code_bindings(context).into_iter().filter(|binding| binding.service_id == service_id).collect()
    }

    fn validate_code_source(
        &self,
        context: &WorkspaceContext,
        source: &CodeSource,
        current_id: Option<&str>,
    ) -> Result<(), CoreError> {
        if source.display_name.trim().is_empty()
            || source.provider.trim().is_empty()
            || source.canonical_key.trim().is_empty()
        {
            return Err(CoreError::InvalidCodeSource);
        }
        if source.root_locator.trim().is_empty() {
            return Err(CoreError::InvalidCodeSource);
        }
        ensure_tenant(context, &source.tenant_id)?;
        ensure_workspace(context, &source.workspace_id)?;
        if self.code_sources.values().any(|existing| {
            Some(existing.id.as_str()) != current_id
                && existing.tenant_id == context.tenant_id
                && existing.workspace_id == context.workspace_id
                && existing.canonical_key == source.canonical_key
        }) {
            return Err(CoreError::DuplicateCodeSource);
        }
        Ok(())
    }

    fn validate_repository(
        &self,
        context: &WorkspaceContext,
        repository: &Repository,
        current_id: Option<&str>,
    ) -> Result<(), CoreError> {
        if repository.provider.trim().is_empty()
            || repository.display_name.trim().is_empty()
            || repository.canonical_url.trim().is_empty()
        {
            return Err(CoreError::InvalidRepository);
        }
        ensure_tenant(context, &repository.tenant_id)?;
        ensure_workspace(context, &repository.workspace_id)?;
        if self.repositories.values().any(|existing| {
            Some(existing.id.as_str()) != current_id
                && existing.tenant_id == context.tenant_id
                && existing.workspace_id == context.workspace_id
                && existing.canonical_url == repository.canonical_url
        }) {
            return Err(CoreError::DuplicateRepository);
        }
        Ok(())
    }

    fn validate_code_binding(
        &self,
        context: &WorkspaceContext,
        binding: &ServiceCodeBinding,
        current_id: Option<&str>,
    ) -> Result<(), CoreError> {
        let service = self.services.get(&binding.service_id).ok_or(CoreError::NotFound("service"))?;
        let source = self.code_sources.get(&binding.source_id).ok_or(CoreError::NotFound("code_source"))?;
        ensure_tenant(context, &binding.tenant_id)?;
        ensure_workspace(context, &binding.workspace_id)?;
        ensure_tenant(context, &service.tenant_id)?;
        ensure_workspace(context, &service.workspace_id)?;
        ensure_tenant(context, &source.tenant_id)?;
        ensure_workspace(context, &source.workspace_id)?;
        if binding.path.starts_with('/')
            || binding.path.contains('\\')
            || binding.path.split('/').any(|segment| segment == ".." || segment.is_empty())
            || binding.path.chars().any(char::is_control)
        {
            return Err(CoreError::InvalidCodeBinding);
        }
        if matches!(binding.entity_kind, CodeEntityKind::RepositoryRoot) && !binding.path.is_empty() {
            return Err(CoreError::InvalidCodeBinding);
        }
        let revision_required = matches!(
            binding.revision_kind,
            CodeRevisionKind::Branch | CodeRevisionKind::Tag | CodeRevisionKind::Commit
        );
        if revision_required != binding.revision.as_ref().is_some_and(|value| !value.trim().is_empty()) {
            return Err(CoreError::InvalidCodeBinding);
        }
        if self.code_bindings.values().any(|existing| {
            Some(existing.id.as_str()) != current_id
                && existing.tenant_id == context.tenant_id
                && existing.workspace_id == context.workspace_id
                && existing.service_id == binding.service_id
                && existing.source_id == binding.source_id
                && existing.entity_kind == binding.entity_kind
                && existing.path == binding.path
        }) {
            return Err(CoreError::DuplicateCodeBinding);
        }
        if binding.role == CodeBindingRole::Primary
            && self.code_bindings.values().any(|existing| {
                Some(existing.id.as_str()) != current_id
                    && existing.tenant_id == context.tenant_id
                    && existing.workspace_id == context.workspace_id
                    && existing.service_id == binding.service_id
                    && existing.role == CodeBindingRole::Primary
            })
        {
            return Err(CoreError::DuplicateCodeBinding);
        }
        Ok(())
    }

    pub fn list_groups(&self, context: &WorkspaceContext) -> Vec<&ServiceGroup> {
        self.groups
            .values()
            .filter(|group| group.tenant_id == context.tenant_id && group.workspace_id == context.workspace_id)
            .collect()
    }

    pub fn list_tags(&self, context: &WorkspaceContext) -> Vec<&Tag> {
        self.tags
            .values()
            .filter(|tag| tag.tenant_id == context.tenant_id && tag.workspace_id == context.workspace_id)
            .collect()
    }

    pub fn list_specifications(&self, context: &WorkspaceContext) -> Vec<&Specification> {
        self.specifications
            .values()
            .filter(|specification| {
                specification.tenant_id == context.tenant_id && specification.workspace_id == context.workspace_id
            })
            .collect()
    }

    /// Saves the facts visible in `context` using the durable store.
    pub fn persist(
        &self,
        store: &persistence::SqliteStore,
        context: &WorkspaceContext,
    ) -> Result<(), persistence::PersistenceError> {
        store.persist_core(self, context)
    }

    /// Reconstructs a scoped in-memory view from durable storage.
    pub fn load(
        store: &persistence::SqliteStore,
        context: &WorkspaceContext,
    ) -> Result<Self, persistence::PersistenceError> {
        store.load_core(context)
    }

    fn is_descendant(&self, candidate_id: &str, ancestor_id: &str) -> bool {
        let mut current = self.groups.get(candidate_id);
        while let Some(group) = current {
            if group.id == ancestor_id {
                return true;
            }
            current = group.parent_id.as_deref().and_then(|id| self.groups.get(id));
        }
        false
    }
}

fn validate_ref_track(track: &ServiceRefTrack) -> Result<(), CoreError> {
    let has_name = track.ref_name.as_ref().is_some_and(|name| !name.trim().is_empty());
    if (track.ref_kind == ServiceRefKind::DefaultBranch) == has_name {
        return Err(CoreError::InvalidRefTrack);
    }
    if let Some(name) = &track.ref_name {
        if name.chars().any(char::is_control) {
            return Err(CoreError::InvalidRefTrack);
        }
        match track.ref_kind {
            ServiceRefKind::DefaultBranch => return Err(CoreError::InvalidRefTrack),
            ServiceRefKind::Commit => {
                if !matches!(name.len(), 40 | 64) || !name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(CoreError::InvalidRefTrack);
                }
            }
            ServiceRefKind::Branch | ServiceRefKind::Tag => {
                let invalid = name.is_empty()
                    || name.starts_with('/')
                    || name.ends_with('/')
                    || name.starts_with('.')
                    || name.ends_with('.')
                    || name.ends_with(".lock")
                    || name.contains("..")
                    || name.contains("@{")
                    || name == "@"
                    || name.split('/').any(|part| part.is_empty() || part.starts_with('.') || part.ends_with('.'))
                    || name.chars().any(|character| {
                        character.is_whitespace() || matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
                    });
                if invalid {
                    return Err(CoreError::InvalidRefTrack);
                }
            }
        }
    }
    Ok(())
}

fn validate_specification_ref(binding: &SpecificationCodeBinding) -> Result<(), CoreError> {
    if !binding.path.is_empty() {
        let lower_path = binding.path.to_ascii_lowercase();
        let invalid_path = binding.path.starts_with('/')
            || binding.path.starts_with('\\')
            || binding.path.ends_with('/')
            || binding.path.contains('\\')
            || binding.path.contains("..")
            || binding.path.chars().any(char::is_control)
            || binding.path.chars().any(|character| matches!(character, '~' | '^' | ':' | '?' | '*' | '['))
            || binding.path.split('/').any(|part| part.is_empty() || part == ".");
        if invalid_path || !(lower_path.ends_with(".md") || lower_path.ends_with(".markdown")) {
            return Err(CoreError::InvalidSpecificationCodeBinding);
        }
    }
    if binding.ref_name.is_empty() || binding.ref_name.chars().any(char::is_control) {
        return Err(CoreError::InvalidSpecificationCodeBinding);
    }
    match binding.ref_kind {
        SpecificationRefKind::Commit => {
            if !matches!(binding.ref_name.len(), 40 | 64)
                || !binding.ref_name.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(CoreError::InvalidSpecificationCodeBinding);
            }
        }
        SpecificationRefKind::Branch | SpecificationRefKind::Tag => {
            let name = &binding.ref_name;
            let invalid = name.starts_with('/')
                || name.ends_with('/')
                || name.starts_with('.')
                || name.ends_with('.')
                || name.ends_with(".lock")
                || name.contains("..")
                || name.contains("@{")
                || name == "@"
                || name.split('/').any(|part| part.is_empty() || part.starts_with('.') || part.ends_with('.'))
                || name.chars().any(|character| {
                    character.is_whitespace() || matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
                });
            if invalid {
                return Err(CoreError::InvalidSpecificationCodeBinding);
            }
        }
    }
    Ok(())
}

pub fn normalize_repository_url(raw_url: &str) -> Result<(String, String, String), CoreError> {
    let raw_url = raw_url.trim();
    let ssh_scp_url = raw_url
        .strip_prefix("git@")
        .and_then(|value| value.split_once(':'))
        .map(|(host, path)| format!("ssh://git@{host}/{path}"));
    let parsed =
        url::Url::parse(ssh_scp_url.as_deref().unwrap_or(raw_url)).map_err(|_| CoreError::InvalidRepository)?;
    if !matches!(parsed.scheme(), "https" | "ssh")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty() && parsed.scheme() == "https"
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CoreError::InvalidRepository);
    }
    let host = parsed.host_str().ok_or(CoreError::InvalidRepository)?.to_ascii_lowercase();
    let mut path = parsed.path().trim_matches('/').to_owned();
    if let Some(without_git) = path.strip_suffix(".git") {
        path = without_git.to_owned();
    }
    if path.is_empty() || path.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..") {
        return Err(CoreError::InvalidRepository);
    }
    let authority = parsed.port().map_or(host.clone(), |port| format!("{host}:{port}"));
    let canonical_url = format!("{}://{authority}/{path}", parsed.scheme());
    let provider = match host.as_str() {
        "github.com" => "github",
        "gitlab.com" => "gitlab",
        "bitbucket.org" => "bitbucket",
        _ => "generic_git",
    }
    .to_owned();
    let display_name = path.trim_end_matches('/').rsplit('/').next().unwrap_or(&path).to_owned();
    Ok((canonical_url, provider, display_name))
}

fn ensure_tenant(context: &WorkspaceContext, tenant_id: &str) -> Result<(), CoreError> {
    if context.tenant_id == tenant_id { Ok(()) } else { Err(CoreError::ScopeMismatch) }
}

fn ensure_workspace(context: &WorkspaceContext, workspace_id: &str) -> Result<(), CoreError> {
    if context.workspace_id == workspace_id { Ok(()) } else { Err(CoreError::ScopeMismatch) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(tenant_id: &str, workspace_id: &str) -> WorkspaceContext {
        WorkspaceContext { tenant_id: tenant_id.into(), workspace_id: workspace_id.into(), actor_id: "actor-1".into() }
    }

    fn service(context: &WorkspaceContext, group_id: &str) -> Service {
        Service {
            id: "svc-1".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            group_id: Some(group_id.into()),
            name: "Search API".into(),
            slug: "search-api".into(),
            description: String::new(),
            tag_ids: vec![],
            updated_at: "2026-10-23T00:00:00Z".into(),
            archived: false,
        }
    }

    #[test]
    fn nested_groups_reject_cycles() {
        let context = context("tenant-1", "workspace-1");
        let mut core = Core::default();
        core.create_group(&context, "root".into(), "Root".into(), None).unwrap();
        core.create_group(&context, "child".into(), "Child".into(), Some("root".into())).unwrap();
        assert_eq!(core.move_group(&context, "root", Some("child".into())), Err(CoreError::GroupCycle));
    }

    #[test]
    fn group_order_updates_hierarchy_atomically_and_rejects_cycles() {
        let context = context("tenant-1", "workspace-1");
        let mut core = Core::default();
        core.create_group(&context, "root-a".into(), "Root A".into(), None).unwrap();
        core.create_group(&context, "root-b".into(), "Root B".into(), None).unwrap();
        core.create_group(&context, "child".into(), "Child".into(), Some("root-a".into())).unwrap();

        core.reorder_groups(
            &context,
            &[
                ServiceGroupOrder { id: "root-a".into(), parent_id: None, position: 1 },
                ServiceGroupOrder { id: "root-b".into(), parent_id: None, position: 0 },
                ServiceGroupOrder { id: "child".into(), parent_id: Some("root-b".into()), position: 0 },
            ],
        )
        .unwrap();
        let reordered = core.list_groups(&context);
        let root_a = reordered.iter().find(|group| group.id == "root-a").unwrap();
        let root_b = reordered.iter().find(|group| group.id == "root-b").unwrap();
        let child = reordered.iter().find(|group| group.id == "child").unwrap();
        assert_eq!((root_a.parent_id.as_deref(), root_a.position), (None, 1));
        assert_eq!((root_b.parent_id.as_deref(), root_b.position), (None, 0));
        assert_eq!((child.parent_id.as_deref(), child.position), (Some("root-b"), 0));

        let invalid_cycle = [
            ServiceGroupOrder { id: "root-a".into(), parent_id: Some("child".into()), position: 0 },
            ServiceGroupOrder { id: "root-b".into(), parent_id: None, position: 0 },
            ServiceGroupOrder { id: "child".into(), parent_id: Some("root-a".into()), position: 0 },
        ];
        assert_eq!(core.reorder_groups(&context, &invalid_cycle), Err(CoreError::GroupCycle));
        let after_rejection = core.list_groups(&context);
        assert_eq!(after_rejection.iter().find(|group| group.id == "root-a").unwrap().parent_id, None);
        assert_eq!(
            after_rejection.iter().find(|group| group.id == "child").unwrap().parent_id.as_deref(),
            Some("root-b")
        );
    }

    #[test]
    fn service_creation_checks_tenant_workspace_and_group_scope() {
        let context = context("tenant-1", "workspace-1");
        let other_workspace = super::tests::context("tenant-1", "workspace-2");
        let mut core = Core::default();
        core.create_group(&other_workspace, "other-root".into(), "Other".into(), None).unwrap();
        assert_eq!(core.create_service(&context, service(&context, "other-root")), Err(CoreError::ScopeMismatch));
    }

    #[test]
    fn code_bindings_validate_paths_roles_and_references() {
        let context = context("tenant-1", "workspace-1");
        let mut core = Core::default();
        core.create_service(
            &context,
            Service {
                id: "svc-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: None,
                name: "Search".into(),
                slug: "search".into(),
                description: String::new(),
                tag_ids: vec![],
                updated_at: "2026-11-01T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        core.create_code_source(
            &context,
            CodeSource {
                id: "src-1".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                source_type: CodeSourceType::Git,
                provider: "github".into(),
                display_name: "acme/search".into(),
                canonical_key: "github:42".into(),
                root_locator: "https://github.com/acme/search.git".into(),
                execution_scope: CodeExecutionScope::Server,
                created_at: "2026-11-01T00:00:00Z".into(),
                updated_at: "2026-11-01T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        let binding = ServiceCodeBinding {
            id: "binding-1".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id: "svc-1".into(),
            source_id: "src-1".into(),
            role: CodeBindingRole::Primary,
            entity_kind: CodeEntityKind::Directory,
            path: "services/search".into(),
            revision_kind: CodeRevisionKind::Branch,
            revision: Some("main".into()),
            created_at: "2026-11-01T00:00:00Z".into(),
            updated_at: "2026-11-01T00:00:00Z".into(),
        };
        core.create_code_binding(&context, binding.clone()).unwrap();
        assert_eq!(core.create_code_binding(&context, binding), Err(CoreError::DuplicateCodeBinding));
        let invalid = ServiceCodeBinding {
            id: "binding-2".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id: "svc-1".into(),
            source_id: "src-1".into(),
            role: CodeBindingRole::Additional,
            entity_kind: CodeEntityKind::Directory,
            path: "../escape".into(),
            revision_kind: CodeRevisionKind::DefaultBranch,
            revision: None,
            created_at: "2026-11-01T00:00:00Z".into(),
            updated_at: "2026-11-01T00:00:00Z".into(),
        };
        assert_eq!(core.create_code_binding(&context, invalid), Err(CoreError::InvalidCodeBinding));
    }

    #[test]
    fn repository_link_is_single_per_service_and_tracks_keep_refs_independent() {
        let context = context("tenant-1", "workspace-1");
        let mut core = Core::default();
        core.create_service(
            &context,
            Service {
                id: "svc-code".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: None,
                name: "Payments".into(),
                slug: "payments".into(),
                description: String::new(),
                tag_ids: vec![],
                updated_at: "2026-11-01T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();

        let repository =
            core.create_repository(&context, "repo-1".into(), "https://GitHub.com/acme/payments.git/").unwrap();
        assert_eq!(repository.canonical_url, "https://github.com/acme/payments");
        assert_eq!(repository.provider, "github");
        let reused = core.repository_by_canonical_url(&context, "https://github.com/acme/payments").unwrap();
        assert_eq!(reused.id, repository.id);

        let relation = core.link_service_repository(&context, "relation-1".into(), "svc-code", &repository.id).unwrap();
        let same = core.link_service_repository(&context, "ignored-id".into(), "svc-code", &repository.id).unwrap();
        assert_eq!(same.id, relation.id);

        let commit = "A".repeat(40);
        for (id, ref_kind, ref_name) in [
            ("track-main", ServiceRefKind::Branch, Some("main")),
            ("track-release", ServiceRefKind::Branch, Some("release/v1")),
            ("track-tag", ServiceRefKind::Tag, Some("v1.2.0")),
            ("track-commit", ServiceRefKind::Commit, Some(commit.as_str())),
        ] {
            let track = core
                .create_service_ref_track(
                    &context,
                    ServiceRefTrack {
                        id: id.into(),
                        tenant_id: context.tenant_id.clone(),
                        workspace_id: context.workspace_id.clone(),
                        service_repository_id: relation.id.clone(),
                        ref_kind,
                        ref_name: ref_name.map(str::to_owned),
                        created_at: "2026-11-01T00:00:00Z".into(),
                        updated_at: "2026-11-01T00:00:00Z".into(),
                    },
                )
                .unwrap();
            if ref_kind == ServiceRefKind::Commit {
                assert_eq!(track.ref_name, Some("a".repeat(40)));
            }
        }

        assert_eq!(core.list_service_ref_tracks(&context, &relation.id).len(), 4);
        let duplicate = ServiceRefTrack {
            id: "track-duplicate".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_repository_id: relation.id.clone(),
            ref_kind: ServiceRefKind::Branch,
            ref_name: Some("main".into()),
            created_at: "2026-11-01T00:00:00Z".into(),
            updated_at: "2026-11-01T00:00:00Z".into(),
        };
        assert_eq!(core.create_service_ref_track(&context, duplicate), Err(CoreError::DuplicateRefTrack));

        let other = core.create_repository(&context, "repo-2".into(), "ssh://git@gitlab.com/acme/other.git").unwrap();
        assert_eq!(
            core.link_service_repository(&context, "relation-2".into(), "svc-code", &other.id),
            Err(CoreError::ServiceRepositoryExists)
        );
    }

    #[test]
    fn repository_snapshots_are_scoped_and_deduplicated() {
        let context = context("tenant-1", "workspace-1");
        let other = super::tests::context("tenant-1", "workspace-2");
        let mut core = Core::default();
        let repository = core.create_repository(&context, "repo-1".into(), "https://github.com/acme/api").unwrap();
        let snapshot = RepositorySnapshot {
            id: "snapshot-1".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            repository_id: repository.id.clone(),
            source_ref: "main".into(),
            resolved_commit: "a".repeat(40),
            manifest_hash: "sha256:manifest".into(),
            content_root_ref: None,
            fetched_at: "2026-11-07T00:00:00Z".into(),
            provider_version: "git-cli".into(),
            created_at: "2026-11-07T00:00:00Z".into(),
        };
        assert_eq!(core.record_repository_snapshot(&context, snapshot.clone()).unwrap(), snapshot);
        assert_eq!(
            core.record_repository_snapshot(
                &context,
                RepositorySnapshot { id: "different".into(), ..snapshot.clone() }
            )
            .unwrap()
            .id,
            "snapshot-1"
        );
        assert_eq!(core.list_repository_snapshots(&context, &repository.id).len(), 1);
        assert!(core.repository_snapshot(&other, "snapshot-1").is_none());
        assert_eq!(core.record_repository_snapshot(&other, snapshot), Err(CoreError::NotFound("repository")));
    }

    #[test]
    fn repository_and_ref_track_reject_invalid_values() {
        assert_eq!(normalize_repository_url("file:///etc/passwd"), Err(CoreError::InvalidRepository));
        assert_eq!(
            normalize_repository_url("https://user:secret@github.com/acme/repo"),
            Err(CoreError::InvalidRepository)
        );
        assert_eq!(normalize_repository_url("git@github.com:acme/repo.git").unwrap().0, "ssh://github.com/acme/repo");

        let context = context("tenant-1", "workspace-1");
        let mut core = Core::default();
        core.create_service(
            &context,
            Service {
                id: "svc-code".into(),
                tenant_id: context.tenant_id.clone(),
                workspace_id: context.workspace_id.clone(),
                group_id: None,
                name: "Payments".into(),
                slug: "payments".into(),
                description: String::new(),
                tag_ids: vec![],
                updated_at: "2026-11-01T00:00:00Z".into(),
                archived: false,
            },
        )
        .unwrap();
        let repository = core.create_repository(&context, "repo-1".into(), "https://github.com/acme/payments").unwrap();
        let relation = core.link_service_repository(&context, "relation-1".into(), "svc-code", &repository.id).unwrap();
        let invalid = ServiceRefTrack {
            id: "track-invalid".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_repository_id: relation.id,
            ref_kind: ServiceRefKind::Branch,
            ref_name: Some("../main".into()),
            created_at: "2026-11-01T00:00:00Z".into(),
            updated_at: "2026-11-01T00:00:00Z".into(),
        };
        assert_eq!(core.create_service_ref_track(&context, invalid), Err(CoreError::InvalidRefTrack));
    }

    #[test]
    fn group_parent_and_service_tag_must_belong_to_the_same_tenant() {
        let tenant_one = context("tenant-1", "shared-workspace");
        let tenant_two = context("tenant-2", "shared-workspace");
        let mut core = Core::default();
        core.create_group(&tenant_two, "tenant-two-root".into(), "Tenant two".into(), None).unwrap();
        core.create_tag(&tenant_two, "tenant-two-tag".into(), "Private".into(), "#c00".into()).unwrap();
        core.create_group(&tenant_one, "tenant-one-root".into(), "Tenant one".into(), None).unwrap();

        assert_eq!(
            core.create_group(
                &tenant_one,
                "invalid-child".into(),
                "Invalid child".into(),
                Some("tenant-two-root".into())
            ),
            Err(CoreError::ScopeMismatch),
        );
        assert_eq!(
            core.move_group(&tenant_one, "tenant-one-root", Some("tenant-two-root".into())),
            Err(CoreError::ScopeMismatch),
        );

        let mut cross_tenant_service = service(&tenant_one, "tenant-two-root");
        cross_tenant_service.group_id = None;
        cross_tenant_service.tag_ids = vec!["tenant-two-tag".into()];
        assert_eq!(core.create_service(&tenant_one, cross_tenant_service), Err(CoreError::ScopeMismatch));
    }

    #[test]
    fn specification_requires_service_in_same_scope_and_nonempty_markdown() {
        let context = context("tenant-1", "workspace-1");
        let mut core = Core::default();
        core.create_group(&context, "root".into(), "Root".into(), None).unwrap();
        core.create_service(&context, service(&context, "root")).unwrap();
        let specification = Specification {
            id: "spec-1".into(),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id: "svc-1".into(),
            title: "Overview".into(),
            kind: SpecificationKind::Markdown,
            content: "# Overview\r\n\r\nIntro".into(),
            version: 1,
            content_hash: String::new(),
            updated_at: "2026-10-23T00:00:00Z".into(),
        };
        let created = core.create_markdown_specification(&context, specification.clone()).unwrap();
        assert_eq!(created.content, "# Overview\n\nIntro");
        let updated = core
            .update_markdown_specification(
                &context,
                "spec-1",
                "Updated overview".into(),
                "# Updated\r\n\r\nBody".into(),
            )
            .unwrap();
        assert_eq!(updated.title, "Updated overview");
        assert_eq!(updated.content, "# Updated\n\nBody");
        let other_tenant = super::tests::context("tenant-2", "workspace-1");
        assert_eq!(
            core.update_markdown_specification(&other_tenant, "spec-1", "No access".into(), "# No access".into()),
            Err(CoreError::ScopeMismatch),
        );
        assert_eq!(
            core.update_markdown_specification(&context, "missing", "Missing".into(), "# Missing".into()),
            Err(CoreError::NotFound("specification")),
        );
        let empty = Specification { content: "  ".into(), ..specification };
        assert_eq!(core.create_markdown_specification(&context, empty), Err(CoreError::EmptyContent));
    }
}
