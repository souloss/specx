use specx_types::{ResourceEvent, ResourceEventKind, ResourceRef};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerDescriptor {
    pub id: String,
    pub watched_kinds: Vec<String>,
    pub max_attempts: u32,
}

impl ControllerDescriptor {
    pub fn new<I, S>(id: impl Into<String>, watched_kinds: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self { id: id.into(), watched_kinds: watched_kinds.into_iter().map(Into::into).collect(), max_attempts: 5 }
    }

    fn validate(&self) -> Result<(), RuntimeError> {
        if self.id.trim().is_empty() {
            return Err(RuntimeError::InvalidDescriptor("controller id is empty"));
        }
        if self.watched_kinds.is_empty() || self.watched_kinds.iter().any(|kind| kind.trim().is_empty()) {
            return Err(RuntimeError::InvalidDescriptor("controller must watch at least one resource kind"));
        }
        if self.max_attempts == 0 {
            return Err(RuntimeError::InvalidDescriptor("controller max_attempts must be positive"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconcileRequest {
    pub controller_id: String,
    pub resource: ResourceRef,
    pub generation: u64,
    pub trigger: ResourceEventKind,
    pub deduplication_key: String,
    pub attempt: u32,
}

impl ReconcileRequest {
    pub fn next_attempt(&self) -> Self {
        let mut next = self.clone();
        next.attempt = self.attempt.saturating_add(1);
        next
    }

    fn key(&self) -> RequestKey {
        RequestKey {
            controller_id: self.controller_id.clone(),
            tenant_id: self.resource.tenant_id.clone(),
            workspace_id: self.resource.workspace_id.clone(),
            kind: self.resource.kind.clone(),
            resource_id: self.resource.id.clone(),
            generation: self.generation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerError {
    pub code: String,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
}

impl ControllerError {
    pub fn permanent(code: impl Into<String>) -> Self {
        Self { code: code.into(), retryable: false, retry_after_ms: None }
    }

    pub fn retryable(code: impl Into<String>, retry_after_ms: Option<u64>) -> Self {
        Self { code: code.into(), retryable: true, retry_after_ms }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconcileResult {
    Complete,
    Requeue { after_ms: u64 },
}

pub trait Controller: Send {
    fn descriptor(&self) -> &ControllerDescriptor;
    fn reconcile(&mut self, request: &ReconcileRequest) -> Result<ReconcileResult, ControllerError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunOutcome {
    Complete { request: ReconcileRequest },
    Requeue { request: ReconcileRequest, after_ms: u64 },
    Failed { request: ReconcileRequest, error: ControllerError },
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeError {
    DuplicateController(String),
    InvalidDescriptor(&'static str),
    UnknownController(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateController(id) => write!(formatter, "controller already registered: {id}"),
            Self::InvalidDescriptor(message) => formatter.write_str(message),
            Self::UnknownController(id) => write!(formatter, "controller not registered: {id}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RequestKey {
    controller_id: String,
    tenant_id: String,
    workspace_id: String,
    kind: String,
    resource_id: String,
    generation: u64,
}

struct Entry {
    descriptor: ControllerDescriptor,
    controller: Box<dyn Controller>,
}

/// In-process controller runtime. A host can persist `ReconcileRequest` before
/// enqueueing it and use `RunOutcome` to write back status or schedule retry.
pub struct ControllerRuntime {
    controllers: HashMap<String, Entry>,
    queue: VecDeque<ReconcileRequest>,
    pending: HashSet<RequestKey>,
}

impl Default for ControllerRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerRuntime {
    pub fn new() -> Self {
        Self { controllers: HashMap::new(), queue: VecDeque::new(), pending: HashSet::new() }
    }

    pub fn register(&mut self, controller: Box<dyn Controller>) -> Result<(), RuntimeError> {
        let descriptor = controller.descriptor().clone();
        descriptor.validate()?;
        if self.controllers.contains_key(&descriptor.id) {
            return Err(RuntimeError::DuplicateController(descriptor.id));
        }
        self.controllers.insert(descriptor.id.clone(), Entry { descriptor, controller });
        Ok(())
    }

    /// Routes an event to every controller watching the resource kind.
    /// Repeated events for the same generation are coalesced.
    pub fn notify(&mut self, event: &ResourceEvent) -> Result<usize, RuntimeError> {
        let matching: Vec<String> = self
            .controllers
            .values()
            .filter(|entry| entry.descriptor.watched_kinds.iter().any(|kind| kind == &event.resource.kind))
            .map(|entry| entry.descriptor.id.clone())
            .collect();
        let mut enqueued = 0;
        for controller_id in matching {
            let request = ReconcileRequest {
                controller_id,
                resource: event.resource.clone(),
                generation: event.generation,
                trigger: event.kind,
                deduplication_key: event.deduplication_key.clone(),
                attempt: 1,
            };
            if self.enqueue(request) {
                enqueued += 1;
            }
        }
        Ok(enqueued)
    }

    /// Enqueues a request if the same controller/resource/generation is not
    /// already pending. Callers use this to schedule a returned retry.
    pub fn enqueue(&mut self, request: ReconcileRequest) -> bool {
        let key = request.key();
        if self.pending.insert(key) {
            self.queue.push_back(request);
            true
        } else {
            false
        }
    }

    pub fn pending_len(&self) -> usize {
        self.queue.len()
    }

    pub fn run_once(&mut self) -> Result<Option<RunOutcome>, RuntimeError> {
        let Some(request) = self.queue.pop_front() else {
            return Ok(None);
        };
        self.pending.remove(&request.key());
        let entry = self
            .controllers
            .get_mut(&request.controller_id)
            .ok_or_else(|| RuntimeError::UnknownController(request.controller_id.clone()))?;
        let outcome = match entry.controller.reconcile(&request) {
            Ok(ReconcileResult::Complete) => RunOutcome::Complete { request },
            Ok(ReconcileResult::Requeue { after_ms }) => {
                RunOutcome::Requeue { request: request.next_attempt(), after_ms }
            }
            Err(error) if error.retryable && request.attempt < entry.descriptor.max_attempts => {
                RunOutcome::Requeue { request: request.next_attempt(), after_ms: error.retry_after_ms.unwrap_or(0) }
            }
            Err(error) => RunOutcome::Failed { request, error },
        };
        Ok(Some(outcome))
    }
}

/// Small adapter useful for tests and for hosts that persist event wakeups in
/// their own queue before handing them to `ControllerRuntime`.
pub struct ControllerEventRouter {
    runtime: ControllerRuntime,
}

impl Default for ControllerEventRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerEventRouter {
    pub fn new() -> Self {
        Self { runtime: ControllerRuntime::new() }
    }

    pub fn runtime(&self) -> &ControllerRuntime {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut ControllerRuntime {
        &mut self.runtime
    }

    pub fn notify(&mut self, event: &ResourceEvent) -> Result<usize, RuntimeError> {
        self.runtime.notify(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestController {
        descriptor: ControllerDescriptor,
        failures_left: u32,
    }

    impl Controller for TestController {
        fn descriptor(&self) -> &ControllerDescriptor {
            &self.descriptor
        }

        fn reconcile(&mut self, _request: &ReconcileRequest) -> Result<ReconcileResult, ControllerError> {
            if self.failures_left > 0 {
                self.failures_left -= 1;
                return Err(ControllerError::retryable("temporary_failure", Some(25)));
            }
            Ok(ReconcileResult::Complete)
        }
    }

    fn event(generation: u64, kind: &str) -> ResourceEvent {
        ResourceEvent {
            event_id: format!("event-{generation}"),
            resource: ResourceRef::new("tenant".into(), "workspace".into(), kind, "resource".into()),
            generation,
            kind: ResourceEventKind::Updated,
            deduplication_key: format!("resource-{generation}"),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn routes_and_coalesces_events_by_generation() {
        let mut runtime = ControllerRuntime::new();
        runtime
            .register(Box::new(TestController {
                descriptor: ControllerDescriptor::new("documents", ["Specification"]),
                failures_left: 0,
            }))
            .unwrap();
        assert_eq!(runtime.notify(&event(1, "Specification")).unwrap(), 1);
        assert_eq!(runtime.notify(&event(1, "Specification")).unwrap(), 0);
        assert_eq!(runtime.notify(&event(2, "Specification")).unwrap(), 1);
        assert_eq!(runtime.pending_len(), 2);
    }

    #[test]
    fn retries_transient_failures_with_bounded_attempts() {
        let mut runtime = ControllerRuntime::new();
        runtime
            .register(Box::new(TestController {
                descriptor: ControllerDescriptor {
                    id: "documents".into(),
                    watched_kinds: vec!["Specification".into()],
                    max_attempts: 2,
                },
                failures_left: 2,
            }))
            .unwrap();
        runtime.notify(&event(1, "Specification")).unwrap();
        let first = runtime.run_once().unwrap().unwrap();
        let RunOutcome::Requeue { request, after_ms } = first else { panic!("expected retry") };
        assert_eq!(after_ms, 25);
        assert_eq!(request.attempt, 2);
        assert!(runtime.enqueue(request));
        let second = runtime.run_once().unwrap().unwrap();
        assert!(matches!(second, RunOutcome::Failed { error: ControllerError { retryable: true, .. }, .. }));
    }

    #[test]
    fn only_matching_kinds_are_notified() {
        let mut runtime = ControllerRuntime::new();
        runtime
            .register(Box::new(TestController {
                descriptor: ControllerDescriptor::new("documents", ["Specification"]),
                failures_left: 0,
            }))
            .unwrap();
        assert_eq!(runtime.notify(&event(1, "RepositorySnapshot")).unwrap(), 0);
    }
}
