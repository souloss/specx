use specx_request_agent::{AgentError, AgentRequest, RequestAgent, TargetPolicy};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

fn server(reply: &[u8]) -> (String, std::thread::JoinHandle<String>) {
    let reply = reply.to_vec();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut captured = Vec::new();
        loop {
            let mut bytes = [0; 4096];
            let n = stream.read(&mut bytes).unwrap();
            captured.extend_from_slice(&bytes[..n]);
            if n == 0 || captured.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        stream.write_all(&reply).unwrap();
        String::from_utf8_lossy(&captured).into()
    });
    (url, handle)
}
#[test]
fn denies_loopback_by_default_and_rejects_non_http_credentials_and_injected_headers() {
    let agent = RequestAgent::new(TargetPolicy::default());
    let stop = Arc::new(AtomicBool::new(false));
    for url in ["http://127.0.0.1:81", "http://[::1]:81", "file:///etc/passwd", "http://user:pass@example.com"] {
        assert!(matches!(
            agent.execute(AgentRequest::get(url), stop.clone()),
            Err(AgentError::TargetDenied | AgentError::InvalidRequest)
        ));
    }
    let mut request = AgentRequest::get("https://example.com");
    request.headers.push(("X-Test".into(), "a\r\nInjected: true".into()));
    assert_eq!(agent.execute(request, stop).unwrap_err(), AgentError::InvalidRequest);
}
#[test]
fn sends_headers_and_cookies_and_preserves_duplicate_response_headers_bytes_and_network() {
    let (url, handle) = server(b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nSet-Cookie: a=1; HttpOnly\r\nSet-Cookie: b=2\r\nContent-Length: 4\r\nConnection: close\r\n\r\n\x00\xffab");
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.headers = vec![("Cookie".into(), "session=test".into()), ("X-Test".into(), "yes".into())];
    let result = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"\x00\xffab");
    assert_eq!(result.headers.iter().filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie")).count(), 2);
    assert_eq!(result.network.remote_ip.as_deref(), Some("127.0.0.1"));
    assert!(result.network.tls.is_none());
    assert!(result.timing.total_ms >= result.timing.first_byte_ms);
    assert!(result.response_header_bytes > 0);
    assert!(handle.join().unwrap().contains("Cookie: session=test"));
}
#[test]
fn validates_every_redirect_and_truncates_at_the_configured_limit() {
    let (url, handle) = server(
        b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    assert_eq!(
        agent.execute(AgentRequest::get(&url), Arc::new(AtomicBool::new(false))).unwrap_err(),
        AgentError::TargetDenied
    );
    handle.join().unwrap();
    let (url, handle) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n12345678");
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.max_response_bytes = 4;
    let response = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    assert!(response.truncated);
    assert_eq!(response.body, b"1234");
    handle.join().unwrap();
}
#[test]
fn cancellation_is_observed_before_network_and_timeouts_are_bounded() {
    let agent = RequestAgent::new(TargetPolicy::default());
    let stop = Arc::new(AtomicBool::new(true));
    assert_eq!(
        agent.execute(AgentRequest::get("https://example.com"), stop.clone()).unwrap_err(),
        AgentError::Cancelled
    );
    stop.store(false, Ordering::Relaxed);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.timeout_ms = 100;
    assert_eq!(agent.execute(request, stop).unwrap_err(), AgentError::Timeout);
}

#[test]
fn distinguishes_response_read_decoding_and_connection_failures() {
    let cases: &[(&[u8], AgentError)] = &[
        (b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort", AgentError::ResponseReadFailed),
        (
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\ninvalid-chunk\r\n",
            AgentError::ResponseReadFailed,
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot-gzip",
            AgentError::ResponseDecodingFailed,
        ),
    ];
    for (reply, expected) in cases {
        let (url, handle) = server(reply);
        let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
        assert_eq!(agent.execute(AgentRequest::get(&url), Arc::new(AtomicBool::new(false))).unwrap_err(), *expected);
        handle.join().unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    assert_eq!(
        agent.execute(AgentRequest::get(&url), Arc::new(AtomicBool::new(false))).unwrap_err(),
        AgentError::ConnectFailed
    );
}

#[test]
fn classifies_response_header_overflow_separately_from_connection_failure() {
    let mut reply = b"HTTP/1.1 200 OK\r\n".to_vec();
    for _ in 0..70 {
        reply.extend_from_slice(format!("X-Long: {}\r\n", "a".repeat(1024)).as_bytes());
    }
    reply.extend_from_slice(b"Content-Length: 0\r\nConnection: close\r\n\r\n");
    let (url, handle) = server(&reply);
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    assert_eq!(
        agent.execute(AgentRequest::get(&url), Arc::new(AtomicBool::new(false))).unwrap_err(),
        AgentError::ResponseHeadersTooLarge
    );
    handle.join().unwrap();
}

#[test]
fn actual_headers_include_generated_host_and_binary_upload_content_length() {
    let (url, handle) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.method = "POST".into();
    request.body = vec![0, 255, 3];
    request.headers.push(("Content-Type".into(), "application/octet-stream".into()));
    let response = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    assert!(
        response.request_headers.iter().any(|(key, value)| key.eq_ignore_ascii_case("content-length") && value == "3")
    );
    assert!(response.request_headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("host")));
    assert_eq!(response.request_body_bytes, 3);
    assert_eq!(response.request_method, "POST");
    assert_eq!(response.request_body, [0, 255, 3]);
    let captured = handle.join().unwrap();
    assert!(captured.starts_with("POST / HTTP/"));
    assert_eq!(response.request_header_bytes as usize, captured.find("\r\n\r\n").unwrap() + 4);
}
#[test]
fn empty_post_has_an_explicit_zero_length_and_header_metrics_exclude_the_body() {
    let (url, handle) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.method = "POST".into();
    let response = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    let captured = handle.join().unwrap();
    assert!(
        response
            .request_headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("content-length") && value == "0")
    );
    assert_eq!(response.request_header_bytes as usize, captured.find("\r\n\r\n").unwrap() + 4);
}
#[test]
fn head_reports_only_uploaded_bytes_even_if_the_draft_has_a_body() {
    let (url, handle) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.method = "HEAD".into();
    request.body = b"not uploaded".to_vec();
    let response = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    let actual = handle.join().unwrap();
    assert!(actual.starts_with("HEAD /"));
    assert!(actual.ends_with("\r\n\r\n"));
    assert!(response.request_body.is_empty());
    assert_eq!(response.request_body_bytes, 0);
}
#[test]
fn redirect_strips_cross_origin_credentials_and_rewrites_post_on_303() {
    let (target, target_handle) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let response =
        format!("HTTP/1.1 303 See Other\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let source = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut bytes = [0; 8192];
        let _ = socket.read(&mut bytes);
        socket.write_all(response.as_bytes()).unwrap();
    });
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([origin.clone(), target]).unwrap());
    let mut request = AgentRequest::get(&origin);
    request.method = "POST".into();
    request.body = b"test".to_vec();
    request.headers =
        vec![("Authorization".into(), "Bearer secret".into()), ("Cookie".into(), "session=secret".into())];
    let response = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    let actual = target_handle.join().unwrap();
    source.join().unwrap();
    assert!(actual.starts_with("GET /"));
    assert!(!actual.contains("secret"));
    assert_eq!(response.redirects, 1);
    assert_eq!(response.request_method, "GET");
    assert!(response.request_body.is_empty());
}
#[test]
fn decompresses_gzip_and_preserves_encoded_transfer_size() {
    // gzip of "hello", generated independently with Python gzip.compress.
    let compressed = [31, 139, 8, 0, 0, 0, 0, 0, 2, 3, 203, 72, 205, 201, 201, 7, 0, 134, 166, 16, 54, 5, 0, 0, 0];
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut bytes = [0; 8192];
        let _ = socket.read(&mut bytes);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 25\r\nConnection: close\r\n\r\n")
            .unwrap();
        socket.write_all(&compressed).unwrap();
    });
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let response = agent.execute(AgentRequest::get(&url), Arc::new(AtomicBool::new(false))).unwrap();
    handle.join().unwrap();
    assert_eq!(response.body, b"hello");
    assert_eq!(response.response_body_bytes, 25);
}
#[test]
fn cancellation_interrupts_an_active_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let started = std::time::Instant::now();
    let handle = std::thread::spawn(move || agent.execute(AgentRequest::get(&url), worker_cancelled));
    let (_socket, _) = listener.accept().unwrap();
    cancelled.store(true, Ordering::Relaxed);
    assert_eq!(handle.join().unwrap().unwrap_err(), AgentError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn hidden_headers_can_be_disabled_without_curl_recreating_them() {
    let (url, handle) = server(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let agent = RequestAgent::new(TargetPolicy::with_allowed_origins([url.clone()]).unwrap());
    let mut request = AgentRequest::get(&url);
    request.method = "POST".into();
    request.headers.push(("Content-Type".into(), "text/plain".into()));
    request.headers.push(("X-Keep".into(), "yes".into()));
    request.disabled_headers = ["Accept", "User-Agent", "Accept-Encoding", "Content-Type"].map(str::to_owned).to_vec();
    let response = agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap();
    let captured = handle.join().unwrap().to_ascii_lowercase();
    for name in ["accept", "user-agent", "accept-encoding", "content-type"] {
        assert!(!captured.contains(&format!("\r\n{name}:")), "unexpected {name}: {captured}");
        assert!(!response.request_headers.iter().any(|(key, _)| key.eq_ignore_ascii_case(name)));
    }
    assert!(captured.contains("x-keep: yes"));
}

#[test]
fn rejects_injection_and_unbounded_hidden_header_lists() {
    let agent = RequestAgent::new(TargetPolicy::default());
    for names in [vec!["Accept\r\nX-Injected: yes".into()], vec!["A".repeat(257)], vec!["Accept".into(); 201]] {
        let mut request = AgentRequest::get("https://example.com");
        request.disabled_headers = names;
        assert_eq!(agent.execute(request, Arc::new(AtomicBool::new(false))).unwrap_err(), AgentError::InvalidRequest);
    }
}
