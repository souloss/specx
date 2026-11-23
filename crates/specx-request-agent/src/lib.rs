//! Bounded HTTP execution for authenticated hosts. The agent never logs or persists payloads.
mod tls;
use curl::easy::{Easy, InfoType, List};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::sync::{atomic::AtomicUsize, mpsc};
use std::time::{Duration, Instant};
pub use tls::AgentTls;
static RESOLVING: AtomicUsize = AtomicUsize::new(0);
struct ResolveSlot;
impl Drop for ResolveSlot {
    fn drop(&mut self) {
        RESOLVING.fetch_sub(1, Ordering::Relaxed);
    }
}
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentError {
    InvalidRequest,
    TargetDenied,
    ResolveFailed,
    ConnectFailed,
    ResponseReadFailed,
    ResponseDecodingFailed,
    TlsFailed,
    Timeout,
    Cancelled,
    TooManyRedirects,
    ResponseHeadersTooLarge,
}
#[derive(Clone, Debug, Default)]
pub struct TargetPolicy {
    allowed_origins: Vec<String>,
}
impl TargetPolicy {
    /// Explicit administrator grants permit private targets only at these exact origins.
    pub fn with_allowed_origins(origins: impl IntoIterator<Item = String>) -> Result<Self, AgentError> {
        let mut allowed_origins = Vec::new();
        for origin in origins {
            let url = checked_url(&origin)?;
            if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
                return Err(AgentError::InvalidRequest);
            }
            allowed_origins.push(url.origin().ascii_serialization());
        }
        Ok(Self { allowed_origins })
    }
    fn addresses(&self, url: &Url, timeout: Duration, cancelled: &AtomicBool) -> Result<Vec<IpAddr>, AgentError> {
        let host = url.host_str().ok_or(AgentError::InvalidRequest)?.trim_matches(['[', ']']);
        let port = url.port_or_known_default().ok_or(AgentError::InvalidRequest)?;
        let addresses: Vec<IpAddr> = if let Ok(ip) = host.parse() {
            vec![ip]
        } else {
            RESOLVING
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| (n < 32).then_some(n + 1))
                .map_err(|_| AgentError::ResolveFailed)?;
            let host = host.to_owned();
            let (sender, receiver) = mpsc::sync_channel(1);
            // OS DNS may outlive a request; cap those workers and bound the caller's wait.
            if std::thread::Builder::new()
                .name("specx-agent-dns".into())
                .spawn(move || {
                    let _slot = ResolveSlot;
                    let result = (host.as_str(), port)
                        .to_socket_addrs()
                        .map(|items| items.map(|a| a.ip()).collect())
                        .map_err(|_| AgentError::ResolveFailed);
                    let _ = sender.send(result);
                })
                .is_err()
            {
                RESOLVING.fetch_sub(1, Ordering::Relaxed);
                return Err(AgentError::ResolveFailed);
            }
            let deadline = Instant::now() + timeout;
            loop {
                if cancelled.load(Ordering::Relaxed) {
                    return Err(AgentError::Cancelled);
                }
                let left = deadline.checked_duration_since(Instant::now()).ok_or(AgentError::Timeout)?;
                match receiver.recv_timeout(left.min(Duration::from_millis(50))) {
                    Ok(result) => break result?,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => return Err(AgentError::ResolveFailed),
                }
            }
        };
        if addresses.is_empty() {
            return Err(AgentError::ResolveFailed);
        }
        if !self.allowed_origins.contains(&url.origin().ascii_serialization())
            && addresses.iter().any(|ip| !public_address(*ip))
        {
            return Err(AgentError::TargetDenied);
        }
        Ok(addresses)
    }
}
fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0)
                || (a == 198 && (b == 18 || b == 19))
                || ip.is_documentation()
                || ip.is_broadcast())
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_address(IpAddr::V4(v4));
            }
            let segments = ip.segments();
            !ip.is_loopback()
                && !ip.is_unspecified()
                && segments[0] & 0xe000 == 0x2000
                && !(segments[0] == 0x2001 && (segments[1] == 0xdb8 || segments[1] < 0x200))
        }
    }
}
fn checked_url(value: &str) -> Result<Url, AgentError> {
    let url = Url::parse(value).map_err(|_| AgentError::InvalidRequest)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.len() > 16384
    {
        return Err(AgentError::InvalidRequest);
    }
    Ok(url)
}
#[derive(Clone, Debug)]
pub struct AgentRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub disabled_headers: Vec<String>,
    pub body: Vec<u8>,
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub max_response_bytes: usize,
}
impl AgentRequest {
    pub fn get(url: &str) -> Self {
        Self {
            method: "GET".into(),
            url: url.into(),
            headers: Vec::new(),
            disabled_headers: Vec::new(),
            body: Vec::new(),
            timeout_ms: 20000,
            follow_redirects: true,
            max_response_bytes: 1024 * 1024,
        }
    }
    fn validate(&self) -> Result<(), AgentError> {
        let token = |c: u8| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c);
        if self.method.is_empty()
            || self.method.len() > 32
            || !self.method.bytes().all(token)
            || matches!(self.method.to_uppercase().as_str(), "CONNECT" | "TRACE")
            || self.disabled_headers.len() > 200
            || self.disabled_headers.iter().any(|name| name.is_empty() || name.len() > 256 || !name.bytes().all(token))
            || self.headers.len() > 200
            || self.body.len() > 8 * 1024 * 1024
            || self.timeout_ms == 0
            || self.timeout_ms > 300000
            || self.max_response_bytes == 0
            || self.max_response_bytes > 8 * 1024 * 1024
        {
            return Err(AgentError::InvalidRequest);
        }
        let mut size = 0;
        for (name, value) in &self.headers {
            size += name.len() + value.len();
            if name.is_empty()
                || !name.bytes().all(token)
                || value.bytes().any(|c| c == 0 || c == b'\r' || c == b'\n')
                || [
                    "host",
                    "connection",
                    "content-length",
                    "transfer-encoding",
                    "proxy-authorization",
                    "proxy-connection",
                    "upgrade",
                ]
                .contains(&name.to_ascii_lowercase().as_str())
            {
                return Err(AgentError::InvalidRequest);
            }
        }
        size += self.disabled_headers.iter().map(String::len).sum::<usize>();
        if size > 65536 {
            return Err(AgentError::InvalidRequest);
        }
        checked_url(&self.url)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Default)]
pub struct AgentTiming {
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub tls_ms: f64,
    pub first_byte_ms: f64,
    pub download_ms: f64,
    pub total_ms: f64,
}
#[derive(Clone, Debug, Default)]
pub struct AgentNetwork {
    pub local_ip: Option<String>,
    pub local_port: u16,
    pub remote_ip: Option<String>,
    pub remote_port: u16,
    pub protocol: String,
    pub tls: Option<AgentTls>,
}
#[derive(Clone, Debug)]
pub struct AgentResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub truncated: bool,
    pub url: String,
    pub redirects: usize,
    pub timing: AgentTiming,
    pub network: AgentNetwork,
    pub request_headers: Vec<(String, String)>,
    pub request_method: String,
    pub request_body: Vec<u8>,
    pub request_header_bytes: u64,
    pub request_body_bytes: u64,
    pub response_header_bytes: u64,
    pub response_body_bytes: u64,
}
#[derive(Clone)]
pub struct RequestAgent {
    policy: TargetPolicy,
}
impl RequestAgent {
    pub fn new(policy: TargetPolicy) -> Self {
        Self { policy }
    }
    pub fn execute(&self, mut request: AgentRequest, cancelled: Arc<AtomicBool>) -> Result<AgentResponse, AgentError> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AgentError::Cancelled);
        }
        request.validate()?;
        let started = Instant::now();
        let mut redirects = 0;
        let mut timing = AgentTiming::default();
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err(AgentError::Cancelled);
            }
            let mut url = checked_url(&request.url)?;
            url.set_fragment(None);
            let resolving = Instant::now();
            let remaining =
                Duration::from_millis(request.timeout_ms).checked_sub(started.elapsed()).ok_or(AgentError::Timeout)?;
            let addresses = self.policy.addresses(&url, remaining, &cancelled)?;
            timing.dns_ms += resolving.elapsed().as_secs_f64() * 1000.;
            let remaining =
                Duration::from_millis(request.timeout_ms).checked_sub(started.elapsed()).ok_or(AgentError::Timeout)?;
            let mut response = execute_one(&request, &url, &addresses, remaining, &cancelled)?;
            timing.dns_ms += response.timing.dns_ms;
            timing.connect_ms += response.timing.connect_ms;
            timing.tls_ms += response.timing.tls_ms;
            timing.first_byte_ms += response.timing.first_byte_ms;
            timing.download_ms += response.timing.download_ms;
            if request.follow_redirects
                && matches!(response.status, 301 | 302 | 303 | 307 | 308)
                && let Some((_, location)) =
                    response.headers.iter().find(|(name, _)| name.eq_ignore_ascii_case("location"))
            {
                if redirects >= 10 {
                    return Err(AgentError::TooManyRedirects);
                }
                let next = url.join(location).map_err(|_| AgentError::InvalidRequest)?;
                checked_url(next.as_str())?;
                if next.origin() != url.origin() {
                    request
                        .headers
                        .retain(|(name, _)| !["authorization", "cookie"].contains(&name.to_ascii_lowercase().as_str()));
                }
                if response.status == 303 && request.method != "HEAD"
                    || matches!(response.status, 301 | 302) && request.method == "POST"
                {
                    request.method = "GET".into();
                    request.body.clear();
                    request.headers.retain(|(name, _)| !name.eq_ignore_ascii_case("content-type"));
                }
                request.url = next.into();
                redirects += 1;
                continue;
            }
            response.redirects = redirects;
            response.timing.dns_ms = timing.dns_ms;
            response.timing.connect_ms = timing.connect_ms;
            response.timing.tls_ms = timing.tls_ms;
            response.timing.first_byte_ms = timing.first_byte_ms;
            response.timing.download_ms = timing.download_ms;
            response.timing.total_ms = started.elapsed().as_secs_f64() * 1000.;
            return Ok(response);
        }
    }
}
fn execute_one(
    request: &AgentRequest,
    url: &Url,
    addresses: &[IpAddr],
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<AgentResponse, AgentError> {
    let mut easy = Easy::new();
    let setup = |_: curl::Error| AgentError::InvalidRequest;
    easy.url(url.as_str()).map_err(setup)?;
    easy.proxy("").map_err(setup)?;
    easy.timeout(timeout).map_err(setup)?;
    easy.connect_timeout(timeout.min(Duration::from_secs(20))).map_err(setup)?;
    easy.progress(true).map_err(setup)?;
    easy.verbose(true).map_err(setup)?;
    easy.accept_encoding("").map_err(setup)?;
    easy.useragent("SpecX-Request-Agent/1.0").map_err(setup)?;
    let mut resolve = List::new();
    let host = url.host_str().ok_or(AgentError::InvalidRequest)?;
    let ips = addresses
        .iter()
        .map(|ip| match ip {
            IpAddr::V4(_) => ip.to_string(),
            IpAddr::V6(_) => format!("[{ip}]"),
        })
        .collect::<Vec<_>>()
        .join(",");
    resolve
        .append(&format!("{host}:{}:{ips}", url.port_or_known_default().ok_or(AgentError::InvalidRequest)?))
        .map_err(setup)?;
    easy.resolve(resolve).map_err(setup)?;
    let mut list = List::new();
    for (name, value) in &request.headers {
        if !request.disabled_headers.iter().any(|disabled| name.eq_ignore_ascii_case(disabled)) {
            list.append(&format!("{name}: {value}")).map_err(setup)?;
        }
    }
    // An empty cURL header suppresses both explicit and native defaults on each hop.
    for name in &request.disabled_headers {
        list.append(&format!("{name}:")).map_err(setup)?;
    }
    list.append("Expect:").map_err(setup)?;
    easy.http_headers(list).map_err(setup)?;
    if !request.body.is_empty() || matches!(request.method.as_str(), "POST" | "PUT" | "PATCH") {
        easy.post_fields_copy(&request.body).map_err(setup)?;
    }
    if request.method == "HEAD" {
        easy.nobody(true).map_err(setup)?;
    }
    easy.custom_request(&request.method).map_err(setup)?;
    let mut headers = Vec::new();
    let mut status_text = String::new();
    let mut protocol = String::new();
    let mut body = Vec::new();
    let mut truncated = false;
    let mut header_size = 0;
    let mut headers_overflow = false;
    let mut sent_headers = Vec::new();
    let mut sent_body = Vec::new();
    let mut tls_details = None;
    let raw = easy.raw();
    let performed = {
        let mut transfer = easy.transfer();
        transfer
            .debug_function(|kind, data| {
                if matches!(kind, InfoType::HeaderOut) {
                    sent_headers.clear();
                    sent_body.clear();
                    for line in String::from_utf8_lossy(data).lines().skip(1) {
                        if let Some((name, value)) = line.split_once(':') {
                            sent_headers.push((name.to_owned(), value.trim().to_owned()));
                        }
                    }
                } else if matches!(kind, InfoType::DataOut) {
                    // Plain upload bytes, before TLS framing. HEAD and early
                    // responses can suppress all or part of the editor body.
                    sent_body.extend_from_slice(data);
                }
            })
            .map_err(setup)?;
        transfer.progress_function(|_, _, _, _| !cancelled.load(Ordering::Relaxed)).map_err(setup)?;
        transfer
            .header_function(|bytes| {
                header_size += bytes.len();
                if header_size > 65536 {
                    headers_overflow = true;
                    return false;
                }
                let line = String::from_utf8_lossy(bytes);
                let line = line.trim_end_matches(['\r', '\n']);
                if line.starts_with("HTTP/") {
                    if url.scheme() == "https" {
                        tls_details = tls::collect(raw);
                    }
                    headers.clear();
                    let mut parts = line.splitn(3, ' ');
                    protocol = parts.next().unwrap_or_default().into();
                    parts.next();
                    status_text = parts.next().unwrap_or_default().into();
                } else if let Some((name, value)) = line.split_once(':') {
                    headers.push((name.into(), value.trim().into()));
                }
                true
            })
            .map_err(setup)?;
        transfer
            .write_function(|bytes| {
                let n = bytes.len().min(request.max_response_bytes.saturating_sub(body.len()));
                body.extend_from_slice(&bytes[..n]);
                if n < bytes.len() {
                    truncated = true;
                    Ok(0)
                } else {
                    Ok(n)
                }
            })
            .map_err(setup)?;
        transfer.perform()
    };
    if cancelled.load(Ordering::Relaxed) {
        return Err(AgentError::Cancelled);
    }
    if headers_overflow {
        return Err(AgentError::ResponseHeadersTooLarge);
    }
    if let Err(error) = performed
        && !truncated
    {
        return Err(if error.is_operation_timedout() {
            AgentError::Timeout
        } else if error.is_ssl_connect_error() || error.is_peer_failed_verification() {
            AgentError::TlsFailed
        } else if error.is_couldnt_resolve_host() {
            AgentError::ResolveFailed
        } else if error.is_bad_content_encoding() {
            AgentError::ResponseDecodingFailed
        } else if error.is_partial_file()
            || error.is_recv_error()
            || error.is_got_nothing()
            || error.is_http2_error()
            || error.is_http2_stream_error()
            || error.code() == curl_sys::CURLE_FTP_WEIRD_SERVER_REPLY
        {
            AgentError::ResponseReadFailed
        } else {
            AgentError::ConnectFailed
        });
    }
    let ms = |d: Result<Duration, curl::Error>| d.unwrap_or_default().as_secs_f64() * 1000.;
    let dns = ms(easy.namelookup_time());
    let connect = ms(easy.connect_time());
    let tls = ms(easy.appconnect_time());
    let first = ms(easy.starttransfer_time());
    let total = ms(easy.total_time());
    Ok(AgentResponse {
        status: easy.response_code().unwrap_or(0) as u16,
        status_text,
        headers,
        body,
        truncated,
        url: url.to_string(),
        redirects: 0,
        timing: AgentTiming {
            dns_ms: dns,
            connect_ms: (connect - dns).max(0.),
            tls_ms: if tls > 0. { (tls - connect).max(0.) } else { 0. },
            first_byte_ms: (first - tls.max(connect)).max(0.),
            download_ms: (total - first).max(0.),
            total_ms: total,
        },
        network: AgentNetwork {
            local_ip: easy.local_ip().ok().flatten().map(str::to_owned),
            local_port: easy.local_port().unwrap_or(0),
            remote_ip: easy.primary_ip().ok().flatten().map(str::to_owned),
            remote_port: easy.primary_port().unwrap_or(0),
            protocol,
            tls: tls_details,
        },
        request_headers: sent_headers,
        request_method: request.method.clone(),
        request_body_bytes: sent_body.len() as u64,
        request_body: sent_body,
        // CURLINFO_REQUEST_SIZE includes the uploaded body, unlike HEADER_SIZE.
        request_header_bytes: easy.request_size().unwrap_or(0).saturating_sub(easy.upload_size().unwrap_or(0.0) as u64),
        response_header_bytes: easy.header_size().unwrap_or(0),
        response_body_bytes: easy.download_size().unwrap_or(0.) as u64,
    })
}
