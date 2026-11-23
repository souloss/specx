//! Workspace-scoped wire boundary. No target credentials, bodies or cookies are stored.
use crate::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use specx_request_agent::{AgentError, AgentRequest, RequestAgent, TargetPolicy};
use std::collections::HashMap;
use std::sync::{Mutex as StdMutex, atomic::AtomicBool};
use tokio::sync::Semaphore;

type RequestKey = (String, String, String, String, String);
pub(crate) struct AgentRuntime {
    executor: RequestAgent,
    slots: Arc<Semaphore>,
    active: StdMutex<HashMap<RequestKey, Arc<AtomicBool>>>,
}
impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new(Vec::new()).expect("empty target policy is valid")
    }
}
impl AgentRuntime {
    pub fn new(origins: Vec<String>) -> Result<Self, AgentError> {
        Ok(Self {
            executor: RequestAgent::new(TargetPolicy::with_allowed_origins(origins)?),
            slots: Arc::new(Semaphore::new(16)),
            active: StdMutex::new(HashMap::new()),
        })
    }
}
struct ActiveRequest {
    runtime: Arc<AgentRuntime>,
    key: RequestKey,
    cancelled: Arc<AtomicBool>,
}
impl Drop for ActiveRequest {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Ok(mut active) = self.runtime.active.lock() {
            active.remove(&self.key);
        }
    }
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AgentRequestDto {
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    disabled_headers: Vec<String>,
    #[serde(default)]
    body_base64: String,
    #[serde(default = "timeout")]
    timeout_ms: u64,
    #[serde(default = "yes")]
    follow_redirects: bool,
}
fn timeout() -> u64 {
    20000
}
fn yes() -> bool {
    true
}
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTimingDto {
    dns_ms: f64,
    connect_ms: f64,
    tls_ms: f64,
    first_byte_ms: f64,
    download_ms: f64,
    total_ms: f64,
}
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTlsDto {
    protocol: Option<String>,
    cipher: Option<String>,
    certificate_common_name: Option<String>,
    issuer_common_name: Option<String>,
    expires_at: Option<String>,
}
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentNetworkDto {
    local_ip: Option<String>,
    local_port: u16,
    remote_ip: Option<String>,
    remote_port: u16,
    protocol: String,
    tls: Option<AgentTlsDto>,
}
#[derive(Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentResponseDto {
    status: u16,
    status_text: String,
    headers: Vec<(String, String)>,
    body_base64: String,
    truncated: bool,
    url: String,
    redirects: usize,
    timing: AgentTimingDto,
    network: AgentNetworkDto,
    request_headers: Vec<(String, String)>,
    request_method: String,
    request_body_base64: String,
    request_header_bytes: u64,
    request_body_bytes: u64,
    response_header_bytes: u64,
    response_body_bytes: u64,
}
fn error(error: AgentError) -> ErrorKind {
    match error {
        AgentError::InvalidRequest => ErrorKind::InvalidInput,
        AgentError::TargetDenied => ErrorKind::AgentTargetDenied,
        AgentError::Timeout => ErrorKind::AgentTimeout,
        AgentError::Cancelled => ErrorKind::AgentCancelled,
        AgentError::TlsFailed => ErrorKind::AgentTlsFailed,
        AgentError::TooManyRedirects => ErrorKind::AgentRedirectLimit,
        AgentError::ResolveFailed => ErrorKind::AgentResolveFailed,
        AgentError::ConnectFailed => ErrorKind::AgentConnectionFailed,
        AgentError::ResponseReadFailed => ErrorKind::AgentResponseReadFailed,
        AgentError::ResponseDecodingFailed => ErrorKind::AgentResponseDecodingFailed,
        AgentError::ResponseHeadersTooLarge => ErrorKind::AgentResponseHeadersTooLarge,
    }
}
async fn scope(state: &ApiState, headers: &HeaderMap, service: &str, id: &str) -> Result<RequestKey, ErrorKind> {
    let context = request_context(headers).map_err(|_| ErrorKind::Unauthorized)?;
    if id.is_empty() || id.len() > 100 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
        return Err(ErrorKind::InvalidInput);
    }
    let mut inner = state.inner.lock().await;
    let core = load_core(&mut inner, &context).map_err(|_| ErrorKind::Internal)?;
    if !core.list_services(&context).iter().any(|item| item.id == service) {
        return Err(ErrorKind::NotFound);
    }
    Ok((context.tenant_id, context.workspace_id, context.actor_id, service.into(), id.into()))
}
#[utoipa::path(post, path="/services/{service_id}/openapi/agent/requests/{request_id}", params(("service_id"=String,Path),("request_id"=String,Path)), request_body=AgentRequestDto, responses((status=401,description="Authentication required",body=ErrorResponse),(status=404,description="Service not found",body=ErrorResponse),(status=413,description="Upload exceeds the body limit",body=ErrorResponse),(status=200,description="Bounded Agent result, including all response headers and raw bytes",body=AgentResponseDto),(status=403,description="Forbidden or target denied by host policy",body=ErrorResponse),(status=409,description="Request ID active or cancelled",body=ErrorResponse),(status=422,description="Invalid request",body=ErrorResponse),(status=429,description="Agent at capacity",body=ErrorResponse),(status=502,description="DNS resolution, connection, response read or decompression, header limit, TLS or redirect failure",body=ErrorResponse),(status=504,description="Request timeout",body=ErrorResponse)), tag="openapi")]
pub(crate) async fn execute_agent_request(
    State(state): State<ApiState>,
    Path((service, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(input): Json<AgentRequestDto>,
) -> Result<Response, ErrorKind> {
    let key = scope(&state, &headers, &service, &id).await?;
    if input.body_base64.len() > 12 * 1024 * 1024 {
        return Err(ErrorKind::PayloadTooLarge);
    }
    let body = STANDARD.decode(input.body_base64).map_err(|_| ErrorKind::InvalidInput)?;
    let request = AgentRequest {
        method: input.method.to_uppercase(),
        url: input.url,
        headers: input.headers,
        disabled_headers: input.disabled_headers,
        body,
        timeout_ms: input.timeout_ms,
        follow_redirects: input.follow_redirects,
        max_response_bytes: 1024 * 1024,
    };
    let permit = state.request_agent.slots.clone().try_acquire_owned().map_err(|_| ErrorKind::RateLimited)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.request_agent.active.lock().map_err(|_| ErrorKind::Internal)?;
        if active.contains_key(&key) {
            return Err(ErrorKind::Conflict);
        }
        active.insert(key.clone(), cancelled.clone());
    }
    let guard = ActiveRequest { runtime: state.request_agent.clone(), key, cancelled: cancelled.clone() };
    let executor = state.request_agent.executor.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        executor.execute(request, cancelled)
    })
    .await
    .map_err(|_| ErrorKind::Internal)?
    .map_err(|cause| {
        // The typed category contains no URL, headers, credentials or payload.
        tracing::warn!(reason = ?cause, "request Agent failed");
        error(cause)
    })?;
    drop(guard);
    let value = AgentResponseDto {
        status: result.status,
        status_text: result.status_text,
        headers: result.headers,
        body_base64: STANDARD.encode(result.body),
        truncated: result.truncated,
        url: result.url,
        redirects: result.redirects,
        timing: AgentTimingDto {
            dns_ms: result.timing.dns_ms,
            connect_ms: result.timing.connect_ms,
            tls_ms: result.timing.tls_ms,
            first_byte_ms: result.timing.first_byte_ms,
            download_ms: result.timing.download_ms,
            total_ms: result.timing.total_ms,
        },
        network: AgentNetworkDto {
            local_ip: result.network.local_ip,
            local_port: result.network.local_port,
            remote_ip: result.network.remote_ip,
            remote_port: result.network.remote_port,
            protocol: result.network.protocol,
            tls: result.network.tls.map(|tls| AgentTlsDto {
                protocol: tls.protocol,
                cipher: tls.cipher,
                certificate_common_name: tls.certificate_common_name,
                issuer_common_name: tls.issuer_common_name,
                expires_at: tls.expires_at,
            }),
        },
        request_headers: result.request_headers,
        request_method: result.request_method,
        request_body_base64: STANDARD.encode(result.request_body),
        request_header_bytes: result.request_header_bytes,
        request_body_bytes: result.request_body_bytes,
        response_header_bytes: result.response_header_bytes,
        response_body_bytes: result.response_body_bytes,
    };
    let mut response = Json(value).into_response();
    response.headers_mut().insert("cache-control", HeaderValue::from_static("no-store"));
    Ok(response)
}
#[utoipa::path(delete, path="/services/{service_id}/openapi/agent/requests/{request_id}", params(("service_id"=String,Path),("request_id"=String,Path)), responses((status=401,description="Authentication required",body=ErrorResponse),(status=404,description="Service not found",body=ErrorResponse),(status=422,description="Invalid request ID",body=ErrorResponse),(status=204,description="Cancels only this actor's active request; idempotent"),(status=403,description="Workspace authorization required",body=ErrorResponse)), tag="openapi")]
pub(crate) async fn cancel_agent_request(
    State(state): State<ApiState>,
    Path((service, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ErrorKind> {
    let key = scope(&state, &headers, &service, &id).await?;
    if let Some(cancelled) = state.request_agent.active.lock().map_err(|_| ErrorKind::Internal)?.get(&key) {
        cancelled.store(true, Ordering::Relaxed);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_distinct_network_failure_identities() {
        for (cause, expected) in [
            (AgentError::ResolveFailed, ErrorKind::AgentResolveFailed),
            (AgentError::ConnectFailed, ErrorKind::AgentConnectionFailed),
            (AgentError::ResponseReadFailed, ErrorKind::AgentResponseReadFailed),
            (AgentError::ResponseDecodingFailed, ErrorKind::AgentResponseDecodingFailed),
            (AgentError::ResponseHeadersTooLarge, ErrorKind::AgentResponseHeadersTooLarge),
            (AgentError::TlsFailed, ErrorKind::AgentTlsFailed),
            (AgentError::Timeout, ErrorKind::AgentTimeout),
        ] {
            assert_eq!(error(cause), expected);
        }
    }
}
