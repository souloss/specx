//! Service-centered domain model for specX.
//!
//! This crate owns workspace facts and their invariants. UI session state such as
//! open tabs and filters belongs to the host application.

use std::collections::{HashMap, HashSet};

pub use specx_types::{
    CodeBindingRole, CodeEntityKind, CodeExecutionScope, CodeRevisionKind, CodeSource, CodeSourceType, Id, Service,
    ServiceCodeBinding, ServiceGroup, ServiceGroupOrder, Specification, SpecificationKind, Tag, Workspace,
    WorkspaceContext,
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
    HasDependents(&'static str),
}

#[derive(Clone, Default)]
pub struct Core {
    groups: HashMap<Id, ServiceGroup>,
    tags: HashMap<Id, Tag>,
    services: HashMap<Id, Service>,
    code_sources: HashMap<Id, CodeSource>,
    code_bindings: HashMap<Id, ServiceCodeBinding>,
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
        self.specifications.retain(|_, specification| specification.service_id != service_id);
        Ok(service)
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
        specification.title = title;
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
        self.specifications.remove(specification_id);
        Ok(specification)
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
