use super::*;
use axum::body::Body;
use http_body_util::BodyExt;
use std::sync::OnceLock;
use tower::ServiceExt;

fn headers(request: axum::http::request::Builder) -> axum::http::request::Builder {
    request
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-1")
}

#[tokio::test]
async fn cross_origin_writes_are_rejected_while_same_origin_writes_succeed() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let cross_origin = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx.local")
        .header(ORIGIN, "https://attacker.local")
        .header("sec-fetch-site", "cross-site")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"blocked","name":"Blocked"}"#))
        .unwrap();
    let response = app.clone().oneshot(cross_origin).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["code"], "origin_rejected");

    let cross_scheme = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx.local")
        .header(ORIGIN, "http://specx.local")
        .header("sec-fetch-site", "cross-site")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"blocked-scheme","name":"Blocked scheme"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(cross_scheme).await.unwrap().status(), StatusCode::FORBIDDEN);

    let same_origin = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx.local")
        .header(ORIGIN, "https://specx.local")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"allowed","name":"Allowed"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(same_origin).await.unwrap().status(), StatusCode::OK);

    let read = headers(axum::http::Request::get("/catalog"))
        .header(HOST, "specx.local")
        .header(ORIGIN, "https://attacker.local")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(read).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("access-control-allow-origin"));
}

#[tokio::test]
async fn configured_public_origin_allows_proxy_host_and_rejects_other_origins() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap())
        .with_public_origin("https://specx.example.com/")
        .unwrap();
    let app = router(state);
    let proxied_same_origin = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx-api:4224")
        .header(ORIGIN, "https://specx.example.com")
        .header("sec-fetch-site", "same-origin")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"proxy-allowed","name":"Proxy allowed"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(proxied_same_origin).await.unwrap().status(), StatusCode::OK);

    let internal_host_origin = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx-api:4224")
        .header(ORIGIN, "https://specx-api:4224")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"internal-origin","name":"Internal origin"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(internal_host_origin).await.unwrap().status(), StatusCode::FORBIDDEN);

    let duplicate_origin = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx-api:4224")
        .header(ORIGIN, "https://specx.example.com")
        .header(ORIGIN, "https://specx.example.com")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"duplicate-origin","name":"Duplicate origin"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(duplicate_origin).await.unwrap().status(), StatusCode::FORBIDDEN);

    let cross_site_metadata = headers(axum::http::Request::post("/groups"))
        .header(HOST, "specx-api:4224")
        .header(ORIGIN, "https://specx.example.com")
        .header("sec-fetch-site", "cross-site")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"cross-site","name":"Cross site"}"#))
        .unwrap();
    assert_eq!(app.oneshot(cross_site_metadata).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[test]
fn public_origin_parser_rejects_non_origins_and_normalizes_default_ports() {
    assert!(parse_public_origin("http://example.com:81").is_some());
    assert_eq!(parse_public_origin("HTTPS://Example.com:443"), parse_public_origin("https://example.com/"));
    assert!(parse_public_origin("https://example.com/specx").is_none());
    assert!(parse_public_origin("https://example.com?next=/").is_none());
    assert!(parse_public_origin("https://user@example.com").is_none());
    assert!(parse_public_origin("ftp://example.com").is_none());
}

#[tokio::test]
async fn catalog_requires_context_and_persists_created_facts() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state);
    let unauthorized =
        app.clone().oneshot(axum::http::Request::get("/catalog").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let response = headers(axum::http::Request::post("/groups"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(response).await.unwrap().status(), StatusCode::OK);
    let response = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-1","groupId":"root","name":"Gateway","slug":"gateway"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(response).await.unwrap().status(), StatusCode::OK);
    let response = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    let body = app.oneshot(response).await.unwrap().into_body().collect().await.unwrap().to_bytes();
    let catalog: CatalogResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(catalog.services[0].slug, "gateway");
    assert_eq!(catalog.groups.len(), 1);
}

#[tokio::test]
async fn code_sources_and_bindings_validate_scope_paths_and_etags() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state);
    let service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-code","name":"Payments","slug":"payments","tagIds":[]}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(service).await.unwrap().status(), StatusCode::OK);

    let source = headers(axum::http::Request::post("/code-sources"))
        .header("content-type", "application/json")
        .header("idempotency-key", "source-key")
        .body(Body::from(r#"{"id":"src-code","sourceType":"git","provider":"github","displayName":"acme/payments","canonicalKey":"github:acme/payments","rootLocator":"https://github.com/acme/payments.git","executionScope":"server"}"#))
        .unwrap();
    let response = app.clone().oneshot(source).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let source_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();

    let source_update = headers(axum::http::Request::put("/code-sources/src-code"))
        .header(IF_MATCH, &source_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"sourceType":"git","provider":"gitlab","displayName":"acme/payments","canonicalKey":"gitlab:acme/payments","rootLocator":"https://gitlab.com/acme/payments.git","executionScope":"server"}"#))
        .unwrap();
    let response = app.clone().oneshot(source_update).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated_source_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    assert_ne!(updated_source_etag, source_etag);

    let stale_source_update = headers(axum::http::Request::put("/code-sources/src-code"))
        .header(IF_MATCH, &source_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"sourceType":"git","provider":"github","displayName":"stale","canonicalKey":"github:stale","rootLocator":"https://github.com/acme/stale.git","executionScope":"server"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(stale_source_update).await.unwrap().status(), StatusCode::PRECONDITION_FAILED);

    let binding = headers(axum::http::Request::post("/services/svc-code/code-bindings"))
        .header("content-type", "application/json")
        .header("idempotency-key", "binding-key")
        .body(Body::from(r#"{"id":"binding-code","sourceId":"src-code","role":"primary","entityKind":"directory","path":"services/payments","revisionKind":"branch","revision":"main"}"#))
        .unwrap();
    let response = app.clone().oneshot(binding).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let binding_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();

    let binding_update = headers(axum::http::Request::put("/code-bindings/binding-code"))
        .header(IF_MATCH, &binding_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"primary","entityKind":"directory","path":"services/payments/api","revisionKind":"branch","revision":"main"}"#))
        .unwrap();
    let response = app.clone().oneshot(binding_update).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated_binding_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    assert_ne!(updated_binding_etag, binding_etag);

    let stale_binding_update = headers(axum::http::Request::put("/code-bindings/binding-code"))
        .header(IF_MATCH, &binding_etag)
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"role":"primary","entityKind":"directory","path":"stale","revisionKind":"branch","revision":"main"}"#,
        ))
        .unwrap();
    assert_eq!(app.clone().oneshot(stale_binding_update).await.unwrap().status(), StatusCode::PRECONDITION_FAILED);

    let invalid = headers(axum::http::Request::post("/services/svc-code/code-bindings"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"binding-invalid","sourceId":"src-code","role":"additional","entityKind":"directory","path":"../escape","revisionKind":"default_branch","revision":null}"#))
        .unwrap();
    let response = app.clone().oneshot(invalid).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["code"], "invalid_code_binding");

    let bindings = headers(axum::http::Request::get("/services/svc-code/code-bindings")).body(Body::empty()).unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&app.oneshot(bindings).await.unwrap().into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(body["items"][0]["path"], "services/payments/api");
}

#[tokio::test]
async fn services_bind_repositories_and_specifications_bind_independent_refs() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    for (id, slug) in [("svc-one", "one"), ("svc-two", "two")] {
        let create = headers(axum::http::Request::post("/services"))
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"Service {slug}","slug":"{slug}"}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(create).await.unwrap().status(), StatusCode::OK);
    }

    let link = headers(axum::http::Request::post("/services/svc-one/repository"))
        .header("content-type", "application/json")
        .header("idempotency-key", "repo-one-link")
        .body(Body::from(r#"{"repositoryUrl":"https://github.com/acme/platform.git/"}"#))
        .unwrap();
    let linked = app.clone().oneshot(link).await.unwrap();
    assert_eq!(linked.status(), StatusCode::OK);
    let linked: serde_json::Value =
        serde_json::from_slice(&linked.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(linked["repository"]["provider"], "github");
    assert_eq!(linked["repository"]["canonicalUrl"], "https://github.com/acme/platform");
    let relation_id = linked["id"].as_str().unwrap().to_owned();
    let repository_id = linked["repository"]["id"].as_str().unwrap().to_owned();

    let replay = headers(axum::http::Request::post("/services/svc-one/repository"))
        .header("content-type", "application/json")
        .header("idempotency-key", "repo-one-link")
        .body(Body::from(r#"{"repositoryUrl":"https://github.com/acme/platform.git/"}"#))
        .unwrap();
    let response = app.clone().oneshot(replay).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let replay: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(replay["id"], relation_id);

    let shared = headers(axum::http::Request::post("/services/svc-two/repository"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"repositoryUrl":"https://github.com/acme/platform"}"#))
        .unwrap();
    let shared = app.clone().oneshot(shared).await.unwrap();
    assert_eq!(shared.status(), StatusCode::OK);
    let shared: serde_json::Value =
        serde_json::from_slice(&shared.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_ne!(shared["id"], relation_id);
    assert_eq!(shared["repository"]["id"], repository_id);

    let too_many = headers(axum::http::Request::post("/services/svc-one/repository"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"repositoryUrl":"https://gitlab.com/acme/other"}"#))
        .unwrap();
    let response = app.clone().oneshot(too_many).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let create_spec = headers(axum::http::Request::post("/specifications"))
        .header("content-type", "application/json")
        .header("idempotency-key", "spec-one")
        .body(Body::from(r##"{"id":"spec-one","serviceId":"svc-one","title":"API","content":"# API"}"##))
        .unwrap();
    assert_eq!(app.clone().oneshot(create_spec).await.unwrap().status(), StatusCode::OK);
    let bind = headers(axum::http::Request::put("/specifications/spec-one/code-binding"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"refKind":"branch","refName":"main"}"#))
        .unwrap();
    let response = app.clone().oneshot(bind).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let binding_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    let binding: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(binding["refKind"], "branch");
    assert_eq!(binding["refName"], "main");
    let update = headers(axum::http::Request::put("/specifications/spec-one/code-binding"))
        .header(IF_MATCH, binding_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"refKind":"tag","refName":"v1.2.0"}"#))
        .unwrap();
    let update_response = app.clone().oneshot(update).await.unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated_binding_etag = update_response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    let invalid = headers(axum::http::Request::put("/specifications/spec-one/code-binding"))
        .header(IF_MATCH, updated_binding_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"refKind":"commit","refName":"latest"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(invalid).await.unwrap().status(), StatusCode::UNPROCESSABLE_ENTITY);
    let get_binding =
        headers(axum::http::Request::get("/specifications/spec-one/code-binding")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(get_binding).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let delete_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    let delete_binding = headers(axum::http::Request::delete("/specifications/spec-one/code-binding"))
        .header(IF_MATCH, delete_etag)
        .body(Body::empty())
        .unwrap();
    let delete_response = app.clone().oneshot(delete_binding).await.unwrap();
    if delete_response.status() != StatusCode::NO_CONTENT {
        let status = delete_response.status();
        let body = delete_response.into_body().collect().await.unwrap().to_bytes();
        panic!("delete binding failed: {status} {}", String::from_utf8_lossy(&body));
    }
    let missing_binding =
        headers(axum::http::Request::get("/specifications/spec-one/code-binding")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(missing_binding).await.unwrap().status(), StatusCode::NOT_FOUND);

    let missing_precondition =
        headers(axum::http::Request::delete("/services/svc-one/repository")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(missing_precondition).await.unwrap().status(), StatusCode::PRECONDITION_REQUIRED);
    let stale = headers(axum::http::Request::delete("/services/svc-one/repository"))
        .header(IF_MATCH, "\"stale\"")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(stale).await.unwrap().status(), StatusCode::PRECONDITION_FAILED);
    let latest = headers(axum::http::Request::get("/services/svc-one/repository")).body(Body::empty()).unwrap();
    let latest = app.clone().oneshot(latest).await.unwrap();
    let latest_etag = latest.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    let unlink = headers(axum::http::Request::delete("/services/svc-one/repository"))
        .header(IF_MATCH, latest_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(unlink).await.unwrap().status(), StatusCode::NO_CONTENT);

    let still_linked = headers(axum::http::Request::get("/services/svc-two/repository")).body(Body::empty()).unwrap();
    assert_eq!(app.oneshot(still_linked).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn repository_managed_specifications_are_created_with_an_immutable_source() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-managed","name":"Managed","slug":"managed"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(service).await.unwrap().status(), StatusCode::OK);
    let repository = headers(axum::http::Request::post("/services/svc-managed/repository"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"repositoryUrl":"https://github.com/acme/managed"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(repository).await.unwrap().status(), StatusCode::OK);

    let create = headers(axum::http::Request::post("/specifications"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"id":"spec-managed","serviceId":"svc-managed","title":"API","content":"# API","source":{"path":"docs/api.md","refKind":"branch","refName":"main"}}"##))
        .unwrap();
    let response = app.clone().oneshot(create).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let specification_etag = response.headers().get(ETAG).unwrap().clone();
    let binding =
        headers(axum::http::Request::get("/specifications/spec-managed/code-binding")).body(Body::empty()).unwrap();
    let binding_body = app.clone().oneshot(binding).await.unwrap().into_body().collect().await.unwrap().to_bytes();
    let binding: serde_json::Value = serde_json::from_slice(&binding_body).unwrap();
    assert_eq!(binding["path"], "docs/api.md");

    let update = headers(axum::http::Request::put("/specifications/spec-managed"))
        .header(IF_MATCH, &specification_etag)
        .header("content-type", "application/json")
        .body(Body::from(r##"{"title":"Changed","content":"# Changed"}"##))
        .unwrap();
    let response = app.clone().oneshot(update).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap()["code"], "managed_specification");

    let binding_etag = binding["etag"].as_str().unwrap();
    let remove = headers(axum::http::Request::delete("/specifications/spec-managed/code-binding"))
        .header(IF_MATCH, binding_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(remove).await.unwrap().status(), StatusCode::CONFLICT);

    let delete = headers(axum::http::Request::delete("/specifications/spec-managed"))
        .header(IF_MATCH, &specification_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(delete).await.unwrap().status(), StatusCode::CONFLICT);
}

#[cfg(unix)]
#[tokio::test]
async fn repository_managed_specifications_sync_from_git_and_dedupe_unchanged_content() {
    static GIT_ENV_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    let _guard = GIT_ENV_LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock().await;
    let repository = tempfile::tempdir().unwrap();
    let source = repository.path().join("source");
    std::fs::create_dir_all(source.join("docs")).unwrap();
    std::fs::write(source.join("docs/api.md"), "# New\r\n").unwrap();
    fn git(directory: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git").arg("-C").arg(directory).args(args).status().unwrap();
        assert!(status.success(), "git command failed: {args:?}");
    }
    git(&source, &["init", "--quiet"]);
    git(&source, &["config", "user.email", "test@example.com"]);
    git(&source, &["config", "user.name", "SpecX Test"]);
    git(&source, &["add", "."]);
    git(&source, &["commit", "--quiet", "-m", "initial"]);
    git(&source, &["branch", "-M", "main"]);

    let wrapper = repository.path().join("git-wrapper.py");
    std::fs::write(
        &wrapper,
        "#!/usr/bin/env python3\nimport os, subprocess, sys\nargs = sys.argv[1:]\nsource = os.environ['SPECX_TEST_GIT_SOURCE']\nif args and args[0] == 'clone': args[-2] = source\nelif len(args) >= 4 and args[0] == 'remote' and args[1] == 'add': args[-1] = source\nraise SystemExit(subprocess.run(['git', *args]).returncode)\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&wrapper).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wrapper, permissions).unwrap();
    let previous_git = std::env::var_os("SPECX_GIT_BIN");
    let previous_source = std::env::var_os("SPECX_TEST_GIT_SOURCE");
    unsafe {
        std::env::set_var("SPECX_GIT_BIN", &wrapper);
        std::env::set_var("SPECX_TEST_GIT_SOURCE", source.as_os_str());
    }

    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-sync","name":"Sync","slug":"sync"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(service).await.unwrap().status(), StatusCode::OK);
    let repository = headers(axum::http::Request::post("/services/svc-sync/repository"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"repositoryUrl":"https://github.com/acme/sync"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(repository).await.unwrap().status(), StatusCode::OK);
    let specification = headers(axum::http::Request::post("/specifications"))
        .header("content-type", "application/json")
        .body(Body::from(
            r##"{"id":"spec-sync","serviceId":"svc-sync","title":"API","content":"# Old","source":{"path":"docs/api.md","refKind":"branch","refName":"main"}}"##,
        ))
        .unwrap();
    assert_eq!(app.clone().oneshot(specification).await.unwrap().status(), StatusCode::OK);

    let sync_request =
        || headers(axum::http::Request::post("/specifications/spec-sync/sync")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(sync_request()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["changed"], true);
    assert_eq!(body["specification"]["content"], "# New\n");
    assert_eq!(body["specification"]["version"], 2);
    assert_eq!(body["providerVersion"], "git-cli");
    assert!(body["snapshotId"].as_str().is_some_and(|value| !value.is_empty()));
    assert!(body["manifestHash"].as_str().is_some_and(|value| value.starts_with("sha256:")));
    let first_commit = body["resolvedCommit"].as_str().unwrap().to_owned();

    let status = app
        .clone()
        .oneshot(
            headers(axum::http::Request::get("/specifications/spec-sync/sync-status")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body: serde_json::Value =
        serde_json::from_slice(&status.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(status_body["resolvedCommit"], first_commit);
    assert_eq!(status_body["manifestHash"], body["manifestHash"]);
    assert_eq!(status_body["changed"], false);

    let response = app.clone().oneshot(sync_request()).await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["changed"], false);
    assert_eq!(body["resolvedCommit"], first_commit);
    assert_eq!(body["specification"]["version"], 2);

    std::fs::write(source.join("docs/api.md"), "# Newer\n").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "--quiet", "-m", "update"]);
    let response = app.oneshot(sync_request()).await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["changed"], true);
    assert_eq!(body["specification"]["content"], "# Newer\n");
    assert_eq!(body["specification"]["version"], 3);
    assert_ne!(body["resolvedCommit"], first_commit);

    unsafe {
        match previous_git {
            Some(value) => std::env::set_var("SPECX_GIT_BIN", value),
            None => std::env::remove_var("SPECX_GIT_BIN"),
        }
        match previous_source {
            Some(value) => std::env::set_var("SPECX_TEST_GIT_SOURCE", value),
            None => std::env::remove_var("SPECX_TEST_GIT_SOURCE"),
        }
    }
}

#[tokio::test]
async fn services_can_be_moved_edited_archived_and_restored() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    for (id, name) in [("root", "Root"), ("other", "Other")] {
        let request = headers(axum::http::Request::post("/groups"))
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"{name}"}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
    let create = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-1","groupId":"root","name":"Gateway","slug":"gateway","description":"edge"}"#))
        .unwrap();
    let created = app.clone().oneshot(create).await.unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let original_etag = created.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();

    let missing_precondition = headers(axum::http::Request::put("/services/svc-1"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"groupId":"other","name":"Gateway API","slug":"gateway","description":"public edge"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(missing_precondition).await.unwrap().status(), StatusCode::PRECONDITION_REQUIRED);

    let update = headers(axum::http::Request::put("/services/svc-1"))
        .header(IF_MATCH, &original_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"groupId":"other","name":"Gateway API","slug":"gateway","description":"public edge"}"#))
        .unwrap();
    let response = app.clone().oneshot(update).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    assert_ne!(updated_etag, original_etag);
    let updated: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(updated["groupId"], "other");
    assert_eq!(updated["description"], "public edge");

    let stale_update = headers(axum::http::Request::put("/services/svc-1"))
        .header(IF_MATCH, original_etag)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"groupId":"root","name":"Stale Gateway","slug":"gateway","description":"stale"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(stale_update).await.unwrap().status(), StatusCode::PRECONDITION_FAILED);

    let archive = headers(axum::http::Request::patch("/services/svc-1"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"archived":true}"#))
        .unwrap();
    let response = app.clone().oneshot(archive).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let archived: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(archived["archived"], true);

    let list = headers(axum::http::Request::get("/services")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(list).await.unwrap();
    let page: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page["total"], 0);

    let list = headers(axum::http::Request::get("/services?includeArchived=true")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(list).await.unwrap();
    let page: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page["total"], 1);

    let restore = headers(axum::http::Request::patch("/services/svc-1"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"archived":false}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(restore).await.unwrap().status(), StatusCode::OK);
    let list = headers(axum::http::Request::get("/services?includeArchived=true")).body(Body::empty()).unwrap();
    let response = app.oneshot(list).await.unwrap();
    let page: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page["total"], 1);

    let context =
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: "default".into(), actor_id: "actor-1".into() };
    assert_eq!(state.inner.lock().await.store.audit_event_count(&context).unwrap(), 6);
}

#[tokio::test]
async fn workspace_membership_is_required_and_revocation_takes_effect() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let owner = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(owner).await.unwrap().status(), StatusCode::OK);

    let member = axum::http::Request::get("/catalog")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    let denied = app.clone().oneshot(member).await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(denied.headers()["x-content-type-options"], "nosniff");
    assert_eq!(denied.headers()["cache-control"], "no-store");

    {
        let inner = state.inner.lock().await;
        assert!(inner.store.revoke_workspace_member("tenant-1", "default", "actor-1").unwrap());
    }
    let owner = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.oneshot(owner).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn workspace_invitations_are_private_authorized_single_use_bearers() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let bootstrap = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(bootstrap).await.unwrap().status(), StatusCode::OK);

    let set_viewer = headers(axum::http::Request::put("/members/actor-2"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(set_viewer).await.unwrap().status(), StatusCode::OK);

    let viewer_list = axum::http::Request::get("/invitations")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_list).await.unwrap().status(), StatusCode::FORBIDDEN);
    let viewer_create = axum::http::Request::post("/invitations")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"new@example.com","role":"editor"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_create).await.unwrap().status(), StatusCode::FORBIDDEN);

    let create = headers(axum::http::Request::post("/invitations"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":" New@Example.com ","role":"editor"}"#))
        .unwrap();
    let response = app.clone().oneshot(create).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let token = created["token"].as_str().unwrap();
    assert_eq!(created["email"], "new@example.com");
    assert_eq!(created["role"], "editor");
    assert!(created.get("tokenHash").is_none());

    let token_hash = format!("sha256:{:x}", Sha256::digest(token.as_bytes()));
    let invitation_context =
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: "default".into(), actor_id: "actor-1".into() };
    let stored = {
        let inner = state.inner.lock().await;
        inner.store.find_workspace_invitation("tenant-1", &token_hash).unwrap().unwrap()
    };
    assert_eq!(stored.token_hash, token_hash);
    assert!(!stored.token_hash.contains(token));

    let list = headers(axum::http::Request::get("/invitations")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(list).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let listed: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert!(listed[0].get("token").is_none());
    assert!(listed[0].get("tokenHash").is_none());

    let revoke_create = headers(axum::http::Request::post("/invitations"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"revoke@example.com","role":"viewer"}"#))
        .unwrap();
    let response = app.clone().oneshot(revoke_create).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let revoke_created: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let revoke_id = revoke_created["id"].as_str().unwrap().to_owned();
    let revoke_token = revoke_created["token"].as_str().unwrap().to_owned();

    let viewer_revoke = axum::http::Request::delete(format!("/invitations/{revoke_id}"))
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_revoke).await.unwrap().status(), StatusCode::FORBIDDEN);
    let revoke = headers(axum::http::Request::delete(format!("/invitations/{revoke_id}"))).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(revoke).await.unwrap().status(), StatusCode::NO_CONTENT);

    let listed = headers(axum::http::Request::get("/invitations")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(listed).await.unwrap();
    let listed: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let revoked = listed.as_array().unwrap().iter().find(|item| item["id"] == revoke_id).unwrap();
    assert!(revoked["revokedAt"].as_str().is_some());
    assert!(revoked.get("token").is_none());
    let revoked_accept = axum::http::Request::post("/invitations/accept")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-actor-id", "actor-4")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": revoke_token}).to_string()))
        .unwrap();
    assert_eq!(app.clone().oneshot(revoked_accept).await.unwrap().status(), StatusCode::NOT_FOUND);

    let wrong_tenant = axum::http::Request::post("/invitations/accept")
        .header("x-specx-tenant-id", "tenant-2")
        .header("x-specx-actor-id", "actor-4")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": token}).to_string()))
        .unwrap();
    assert_eq!(app.clone().oneshot(wrong_tenant).await.unwrap().status(), StatusCode::NOT_FOUND);

    let accepted = axum::http::Request::post("/invitations/accept")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-actor-id", "actor-3")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": token}).to_string()))
        .unwrap();
    let response = app.clone().oneshot(accepted).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let accepted: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(accepted["workspaceId"], "default");
    assert_eq!(accepted["actorId"], "actor-3");
    assert_eq!(accepted["role"], "editor");

    let actor_access = axum::http::Request::get("/access")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-3")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(actor_access).await.unwrap().status(), StatusCode::OK);

    let replay = axum::http::Request::post("/invitations/accept")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-actor-id", "actor-3")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": token}).to_string()))
        .unwrap();
    assert_eq!(app.clone().oneshot(replay).await.unwrap().status(), StatusCode::NOT_FOUND);

    let expired_token = "expired.invitation.token";
    let expired_hash = format!("sha256:{:x}", Sha256::digest(expired_token.as_bytes()));
    let expired = WorkspaceInvitation {
        id: "expired-invitation".into(),
        tenant_id: invitation_context.tenant_id.clone(),
        workspace_id: invitation_context.workspace_id.clone(),
        email: "expired@example.com".into(),
        role: "viewer".into(),
        token_hash: expired_hash,
        expires_at: "2000-01-01T00:00:00+00:00".into(),
        accepted_at: None,
        revoked_at: None,
        created_by: invitation_context.actor_id.clone(),
        created_at: "2000-01-01T00:00:00+00:00".into(),
    };
    let expired_audit = AuditEvent {
        tenant_id: invitation_context.tenant_id.clone(),
        workspace_id: invitation_context.workspace_id.clone(),
        actor_id: invitation_context.actor_id.clone(),
        request_id: "expired-invite-test".into(),
        action: "invite".into(),
        resource_type: "workspace_invitation".into(),
        resource_id: expired.id.clone(),
        created_at: expired.created_at.clone(),
    };
    {
        let inner = state.inner.lock().await;
        assert!(inner.store.create_workspace_invitation(&invitation_context, &expired, &expired_audit).unwrap());
    }
    let expired_accept = axum::http::Request::post("/invitations/accept")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-actor-id", "actor-4")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": expired_token}).to_string()))
        .unwrap();
    assert_eq!(app.clone().oneshot(expired_accept).await.unwrap().status(), StatusCode::NOT_FOUND);
    let no_access = axum::http::Request::get("/access")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-4")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(no_access).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn workspace_directory_is_actor_scoped_and_workspace_creation_requires_owner_role() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let bootstrap = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(bootstrap).await.unwrap().status(), StatusCode::OK);

    let create = headers(axum::http::Request::post("/workspaces"))
        .header("x-request-id", "create-workspace-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Release Team"}"#))
        .unwrap();
    let response = app.clone().oneshot(create).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let created: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let workspace_id = created["id"].as_str().unwrap();
    assert_eq!(created["role"], "owner");

    let directory = headers(axum::http::Request::get("/workspaces")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(directory).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let directory: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(directory["items"].as_array().unwrap().len(), 2);
    assert!(directory["items"].as_array().unwrap().iter().any(|item| item["id"] == workspace_id));

    let grant = headers(axum::http::Request::put("/members/actor-2"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"editor"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(grant).await.unwrap().status(), StatusCode::OK);

    let editor_create = axum::http::Request::post("/workspaces")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Editor Workspace"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(editor_create).await.unwrap().status(), StatusCode::FORBIDDEN);

    let other_tenant = axum::http::Request::get("/workspaces")
        .header("x-specx-tenant-id", "tenant-2")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-1")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(other_tenant).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let other_tenant: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(other_tenant["items"].as_array().unwrap().len(), 1);
    assert_eq!(other_tenant["items"][0]["id"], "default");

    let revoke = headers(axum::http::Request::delete("/members/actor-2")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(revoke).await.unwrap().status(), StatusCode::NO_CONTENT);
    let revoked_directory = axum::http::Request::get("/workspaces")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(revoked_directory).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let revoked_directory: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(revoked_directory["items"].as_array().unwrap().is_empty());
    let revoked_catalog = axum::http::Request::get("/catalog")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(revoked_catalog).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn access_endpoint_reports_the_effective_role_and_rejects_revoked_members() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let bootstrap = headers(axum::http::Request::get("/access")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(bootstrap).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let access: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(access["actorId"], "actor-1");
    assert_eq!(access["workspaceId"], "default");
    assert_eq!(access["role"], "owner");

    let grant = headers(axum::http::Request::put("/members/actor-2"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(grant).await.unwrap().status(), StatusCode::OK);
    let viewer = axum::http::Request::get("/access")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(viewer).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let access: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(access["role"], "viewer");

    let revoke = headers(axum::http::Request::delete("/members/actor-2")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(revoke).await.unwrap().status(), StatusCode::NO_CONTENT);
    let revoked = axum::http::Request::get("/access")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(revoked).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn workspace_roles_control_writes_and_member_administration() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let bootstrap = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(bootstrap).await.unwrap().status(), StatusCode::OK);

    let last_owner_demotion = headers(axum::http::Request::put("/members/actor-1"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(last_owner_demotion).await.unwrap().status(), StatusCode::CONFLICT);

    let grant_viewer = headers(axum::http::Request::put("/members/actor-2"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(grant_viewer).await.unwrap().status(), StatusCode::OK);

    let viewer_read = axum::http::Request::get("/catalog")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_read).await.unwrap().status(), StatusCode::OK);
    let viewer_write = axum::http::Request::post("/groups")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"viewer-group","name":"Viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_write).await.unwrap().status(), StatusCode::FORBIDDEN);
    let viewer_manage = axum::http::Request::put("/members/actor-3")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"editor"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_manage).await.unwrap().status(), StatusCode::FORBIDDEN);

    let promote_editor = headers(axum::http::Request::put("/members/actor-2"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"editor"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(promote_editor).await.unwrap().status(), StatusCode::OK);
    let editor_write = axum::http::Request::post("/groups")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"editor-group","name":"Editor"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(editor_write).await.unwrap().status(), StatusCode::OK);

    let revoke = headers(axum::http::Request::delete("/members/actor-2")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(revoke).await.unwrap().status(), StatusCode::NO_CONTENT);
    let revoked_read = axum::http::Request::get("/catalog")
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(revoked_read).await.unwrap().status(), StatusCode::FORBIDDEN);

    let context =
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: "default".into(), actor_id: "actor-1".into() };
    let inner = state.inner.lock().await;
    assert_eq!(inner.store.audit_event_count(&context).unwrap(), 4);
}

#[tokio::test]
async fn backup_endpoint_rotates_and_restores_only_admin_owned_snapshots() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let bootstrap = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(bootstrap).await.unwrap().status(), StatusCode::OK);

    let created = app
        .clone()
        .oneshot(
            headers(axum::http::Request::post("/backups"))
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created_body: serde_json::Value =
        serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let file_name = created_body["fileName"].as_str().unwrap().to_string();
    assert!(file_name.ends_with(".sqlite"));

    #[cfg(unix)]
    {
        let symlink_name = format!("{}-link.sqlite", file_name.trim_end_matches(".sqlite"));
        std::os::unix::fs::symlink(state.backup_root.join(&file_name), state.backup_root.join(&symlink_name)).unwrap();
        let listed = app
            .clone()
            .oneshot(headers(axum::http::Request::get("/backups")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let listed_body: serde_json::Value =
            serde_json::from_slice(&listed.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(listed_body.as_array().unwrap().len(), 1);
        let symlink_restore =
            headers(axum::http::Request::post(format!("/backups/{symlink_name}/restore"))).body(Body::empty()).unwrap();
        assert_eq!(app.clone().oneshot(symlink_restore).await.unwrap().status(), StatusCode::NOT_FOUND);
        std::fs::remove_file(state.backup_root.join(symlink_name)).unwrap();
    }

    let listed =
        app.clone().oneshot(headers(axum::http::Request::get("/backups")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: serde_json::Value =
        serde_json::from_slice(&listed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(listed_body.as_array().unwrap().len(), 1);

    let grant_viewer = headers(axum::http::Request::put("/members/actor-2"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"role":"viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(grant_viewer).await.unwrap().status(), StatusCode::OK);
    let viewer_restore = axum::http::Request::post(format!("/backups/{file_name}/restore"))
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "actor-2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(viewer_restore).await.unwrap().status(), StatusCode::FORBIDDEN);

    let wrong_scope = axum::http::Request::post(format!("/backups/{file_name}/restore"))
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", "different")
        .header("x-specx-actor-id", "actor-1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(wrong_scope).await.unwrap().status(), StatusCode::FORBIDDEN);

    let owner_restore =
        headers(axum::http::Request::post(format!("/backups/{file_name}/restore"))).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(owner_restore).await.unwrap().status(), StatusCode::OK);

    let metrics =
        app.clone().oneshot(headers(axum::http::Request::get("/metrics")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    let metrics = String::from_utf8(metrics.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(metrics.contains("specx_backup_created_total 1"));
    assert!(metrics.contains("specx_backup_restored_total 1"));
    assert!(metrics.contains("specx_api_ready 1"));
}

#[tokio::test]
async fn readiness_reports_migration_state_and_health_headers() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let response = app
        .oneshot(
            axum::http::Request::get("/ready").header("x-request-id", "ready-check-7").body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-security-policy"],
        "default-src 'none'; frame-ancestors 'none'; base-uri 'none'"
    );
    assert_eq!(response.headers()["x-request-id"], "ready-check-7");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "ready");
    assert_eq!(body["schemaVersion"], 16);
}

#[tokio::test]
async fn business_requests_are_gated_when_storage_is_not_ready() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    state.metrics.ready.store(0, Ordering::Release);
    let app = router(state);

    let response = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(response).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["version"], 1);
    assert_eq!(body["code"], "not_ready");
    assert_eq!(body["messageKey"], "apiError.notReady");

    let health = app.oneshot(axum::http::Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(health.status(), StatusCode::OK);
}

#[tokio::test]
async fn successful_mutations_write_a_scoped_audit_event() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let request = headers(axum::http::Request::post("/groups"))
        .header("x-request-id", "group-create-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-request-id"], "group-create-1");

    let context =
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: "default".into(), actor_id: "actor-1".into() };
    let inner = state.inner.lock().await;
    assert_eq!(inner.store.audit_event_count(&context).unwrap(), 1);
}

#[tokio::test]
async fn expired_invitation_cleanup_updates_metrics() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let bootstrap = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(bootstrap).await.unwrap().status(), StatusCode::OK);

    let create = headers(axum::http::Request::post("/invitations"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"cleanup@example.com","role":"viewer"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(create).await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(state.purge_expired_workspace_invitations("9999-12-31T23:59:59Z").await.unwrap(), 1);

    let list = headers(axum::http::Request::get("/invitations")).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(list).await.unwrap();
    let invitations: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(invitations.as_array().unwrap().len(), 0);

    let metrics = headers(axum::http::Request::get("/metrics")).body(Body::empty()).unwrap();
    let response = app.oneshot(metrics).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("specx_invitations_expired_total 1"));

    assert!(
        record_invitation_cleanup_result(&state.metrics, Err(PersistenceError::new("injected cleanup failure")),)
            .is_err()
    );
    let metrics = headers(axum::http::Request::get("/metrics")).body(Body::empty()).unwrap();
    let response = router(state).oneshot(metrics).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("specx_invitation_cleanup_failures_total 1"));
}

#[tokio::test]
async fn actor_rate_limit_rejects_requests_after_the_window_budget() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let context =
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: "default".into(), actor_id: "actor-1".into() };
    for _ in 0..300 {
        assert!(allow_request(&state, &context).await.unwrap());
    }
    assert!(!allow_request(&state, &context).await.unwrap());
}

#[tokio::test]
async fn catalog_is_restored_after_reopening_the_api_state() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("specx.sqlite");
    let app = router(ApiState::open(&database).unwrap());
    let response = headers(axum::http::Request::post("/groups"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    assert_eq!(app.oneshot(response).await.unwrap().status(), StatusCode::OK);

    let reopened = router(ApiState::open(&database).unwrap());
    let response = headers(axum::http::Request::get("/catalog")).body(Body::empty()).unwrap();
    let response = reopened.oneshot(response).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let catalog: CatalogResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(catalog.groups.len(), 1);
    assert_eq!(catalog.groups[0].name, "Root");
}

#[tokio::test]
async fn invalid_parent_has_a_specific_error_code() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let response = headers(axum::http::Request::post("/groups"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"child","name":"Child","parentId":"missing"}"#))
        .unwrap();
    let response = app.oneshot(response).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap()["code"], "invalid_parent");
}

#[tokio::test]
async fn missing_service_tag_returns_not_found_error_code() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let response = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-1","name":"Gateway","slug":"gateway","tagIds":["missing"]}"#))
        .unwrap();
    let response = app.oneshot(response).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["code"], "not_found");
}

#[tokio::test]
async fn core_error_statuses_use_the_matching_protocol_codes() {
    let cases = [
        (CoreError::ScopeMismatch, StatusCode::FORBIDDEN, "scope_mismatch"),
        (CoreError::DuplicateTag, StatusCode::CONFLICT, "duplicate_tag"),
        (CoreError::EmptyName, StatusCode::UNPROCESSABLE_ENTITY, "empty_name"),
    ];

    for (error, expected_status, expected_code) in cases {
        let response = ApiError::Core(error).into_response();
        assert_eq!(response.status(), expected_status);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["code"], expected_code);
    }
}

#[tokio::test]
async fn openapi_endpoint_matches_the_checked_contract_fixture() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let response = app.oneshot(axum::http::Request::get("/openapi.json").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let document: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/openapi-contract.json")).unwrap();

    assert_eq!(document["info"]["title"], fixture["info"]["title"]);
    assert_eq!(document["info"]["version"], fixture["info"]["version"]);
    assert_eq!(document["servers"][0]["url"], fixture["servers"][0]);

    let actual_paths: Vec<_> = document["paths"].as_object().unwrap().keys().cloned().collect();
    let expected_paths: Vec<_> = fixture["operations"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(actual_paths, expected_paths, "OpenAPI paths");

    for (path, methods) in fixture["operations"].as_object().unwrap() {
        let actual_methods = document["paths"][path].as_object().unwrap();
        let actual_method_names: Vec<_> = actual_methods.keys().cloned().collect();
        let expected_method_names: Vec<_> = methods.as_object().unwrap().keys().cloned().collect();
        assert_eq!(actual_method_names, expected_method_names, "{path} methods");
        for (method, expected_codes) in methods.as_object().unwrap() {
            let actual = document["paths"][path][method]["responses"].as_object().unwrap();
            let actual_codes: Vec<_> = actual.keys().cloned().collect();
            let expected_codes: Vec<_> =
                expected_codes.as_array().unwrap().iter().map(|code| code.as_str().unwrap().to_owned()).collect();
            assert_eq!(actual_codes, expected_codes, "{method} {path} response codes");
        }
    }

    for (path, methods) in fixture["parameters"].as_object().unwrap() {
        for (method, expected_parameters) in methods.as_object().unwrap() {
            let mut actual_parameters: Vec<_> = document["paths"][path][method]["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .map(|parameter| parameter["name"].as_str().unwrap().to_owned())
                .collect();
            actual_parameters.sort();
            let mut expected_parameters: Vec<_> = expected_parameters
                .as_array()
                .unwrap()
                .iter()
                .map(|parameter| parameter.as_str().unwrap().to_owned())
                .collect();
            expected_parameters.sort();
            assert_eq!(actual_parameters, expected_parameters, "{method} {path} parameters");
        }
    }

    for (path, methods) in fixture["responseHeaders"].as_object().unwrap() {
        for (method, responses) in methods.as_object().unwrap() {
            for (status, expected_headers) in responses.as_object().unwrap() {
                let mut actual_headers: Vec<_> = document["paths"][path][method]["responses"][status]["headers"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect();
                actual_headers.sort();
                let mut expected_headers: Vec<_> = expected_headers
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|header| header.as_str().unwrap().to_owned())
                    .collect();
                expected_headers.sort();
                assert_eq!(actual_headers, expected_headers, "{method} {path} {status} headers");
            }
        }
    }

    fn collect_schema_fields(
        document: &serde_json::Value,
        schema: &serde_json::Value,
        required: &mut Vec<String>,
        properties: &mut Vec<String>,
    ) {
        if let Some(reference) = schema["$ref"].as_str() {
            if let Some(name) = reference.rsplit('/').next() {
                collect_schema_fields(document, &document["components"]["schemas"][name], required, properties);
            }
            return;
        }
        if let Some(all_of) = schema["allOf"].as_array() {
            for part in all_of {
                collect_schema_fields(document, part, required, properties);
            }
            return;
        }
        if let Some(fields) = schema["required"].as_array() {
            required.extend(fields.iter().filter_map(|field| field.as_str().map(ToOwned::to_owned)));
        }
        if let Some(fields) = schema["properties"].as_object() {
            properties.extend(fields.keys().cloned());
        }
    }

    for (name, expected) in fixture["schemas"].as_object().unwrap() {
        let schema = &document["components"]["schemas"][name];
        let mut required = Vec::new();
        let mut properties = Vec::new();
        collect_schema_fields(&document, schema, &mut required, &mut properties);
        required.sort();
        properties.sort();
        let mut expected_properties: Vec<_> =
            expected["properties"].as_array().unwrap().iter().map(|field| field.as_str().unwrap().to_owned()).collect();
        expected_properties.sort();
        let mut expected_required: Vec<_> =
            expected["required"].as_array().unwrap().iter().map(|field| field.as_str().unwrap().to_owned()).collect();
        expected_required.sort();
        assert_eq!(required, expected_required, "{name} required fields");
        assert_eq!(properties, expected_properties, "{name} properties");
    }
}

#[tokio::test]
async fn service_listing_searches_filters_and_pages_with_stable_order() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let tag = headers(axum::http::Request::post("/tags"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"id":"tag-core","name":"Core","color":"#64748b"}"##))
        .unwrap();
    assert_eq!(app.clone().oneshot(tag).await.unwrap().status(), StatusCode::OK);

    for (id, slug, description) in [
        ("svc-b", "gateway-public", "public gateway"),
        ("svc-a", "gateway-auth", "authentication gateway"),
        ("svc-c", "worker", "background jobs"),
    ] {
        let body = format!(
            r##"{{"id":"{id}","name":"{slug}","slug":"{slug}","description":"{description}","tagIds":["tag-core"]}}"##
        );
        let request = headers(axum::http::Request::post("/services"))
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }

    let first = headers(axum::http::Request::get("/services?q=gateway&tagId=tag-core&limit=1&offset=0"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(first).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let first: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(first["total"], 2);
    assert_eq!(first["items"][0]["slug"], "gateway-auth");
    assert_eq!(first["nextOffset"], 1);
    assert_eq!(first["query"], "gateway");
    assert_eq!(first["tagIds"][0], "tag-core");

    let second = headers(axum::http::Request::get("/services?q=gateway&tagId=tag-core&limit=1&offset=1"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(second).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let second: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(second["items"][0]["slug"], "gateway-public");
    assert_eq!(second["nextOffset"], serde_json::Value::Null);

    let invalid = headers(axum::http::Request::get("/services?limit=201")).body(Body::empty()).unwrap();
    assert_eq!(app.oneshot(invalid).await.unwrap().status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn catalog_resource_lists_have_stable_pagination_contracts() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    for (id, name) in [("group-b", "Zeta"), ("group-a", "Alpha")] {
        let request = headers(axum::http::Request::post("/groups"))
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"{name}"}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
    for (id, name) in [("tag-b", "Zulu"), ("tag-a", "Alpha")] {
        let request = headers(axum::http::Request::post("/tags"))
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"{name}"}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
    let service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-page","name":"Paged","slug":"paged"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(service).await.unwrap().status(), StatusCode::OK);
    for (id, title) in [("spec-b", "Zeta"), ("spec-a", "Alpha")] {
        let request = headers(axum::http::Request::post("/specifications"))
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r##"{{"id":"{id}","serviceId":"svc-page","title":"{title}","content":"# {title}"}}"##
            )))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }

    for (path, first_id, next_path) in [
        ("/groups?limit=1", "group-b", "/groups?limit=1&offset=1"),
        ("/tags?limit=1", "tag-a", "/tags?limit=1&offset=1"),
        ("/specifications?limit=1", "spec-a", "/specifications?limit=1&offset=1"),
    ] {
        let response =
            app.clone().oneshot(headers(axum::http::Request::get(path)).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(page["total"], 2, "{path}");
        assert_eq!(page["items"][0]["id"], first_id, "{path}");
        assert_eq!(page["nextOffset"], 1, "{path}");
        let response = app
            .clone()
            .oneshot(headers(axum::http::Request::get(next_path)).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{next_path}");
    }
    let invalid =
        app.oneshot(headers(axum::http::Request::get("/groups?limit=201")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn group_order_endpoint_persists_same_level_and_nested_moves() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    for (id, name, parent_id) in
        [("group-a", "Alpha", None), ("group-b", "Beta", None), ("group-child", "Child", Some("group-a"))]
    {
        let parent = parent_id.map(|value| format!(r#","parentId":"{value}""#)).unwrap_or_default();
        let request = headers(axum::http::Request::post("/groups"))
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"{name}"{parent}}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }

    let reorder = headers(axum::http::Request::put("/groups/order"))
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"items":[{"id":"group-a","parentId":null,"position":1},{"id":"group-b","parentId":null,"position":0},{"id":"group-child","parentId":"group-b","position":0}]}"#,
            ))
            .unwrap();
    let response = app.clone().oneshot(reorder).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response =
        app.oneshot(headers(axum::http::Request::get("/groups")).body(Body::empty()).unwrap()).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let child = page["items"].as_array().unwrap().iter().find(|group| group["id"] == "group-child").unwrap();
    assert_eq!(child["parentId"], "group-b");
    assert_eq!(child["position"], 0);
}

#[tokio::test]
async fn catalog_resources_support_update_and_delete_lifecycles() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));

    let create_group = headers(axum::http::Request::post("/groups"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"group-crud","name":"Initial"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(create_group).await.unwrap().status(), StatusCode::OK);
    let update_group = headers(axum::http::Request::put("/groups/group-crud"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Updated"}"#))
        .unwrap();
    let response = app.clone().oneshot(update_group).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&body).unwrap()["name"], "Updated");
    let delete_group = headers(axum::http::Request::delete("/groups/group-crud")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(delete_group).await.unwrap().status(), StatusCode::NO_CONTENT);

    let create_tag = headers(axum::http::Request::post("/tags"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"id":"tag-crud","name":"Initial","color":"#000000"}"##))
        .unwrap();
    assert_eq!(app.clone().oneshot(create_tag).await.unwrap().status(), StatusCode::OK);
    let update_tag = headers(axum::http::Request::put("/tags/tag-crud"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"name":"Updated","color":"#ffffff"}"##))
        .unwrap();
    assert_eq!(app.clone().oneshot(update_tag).await.unwrap().status(), StatusCode::OK);
    let delete_tag = headers(axum::http::Request::delete("/tags/tag-crud")).body(Body::empty()).unwrap();
    assert_eq!(app.clone().oneshot(delete_tag).await.unwrap().status(), StatusCode::NO_CONTENT);

    let create_service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"service-crud","name":"Service","slug":"service-crud"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(create_service).await.unwrap().status(), StatusCode::OK);
    let response =
        app.clone().oneshot(headers(axum::http::Request::get("/services")).body(Body::empty()).unwrap()).await.unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let service_page: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let service_etag = service_page["items"][0]["etag"].as_str().unwrap().to_owned();

    let create_specification = headers(axum::http::Request::post("/specifications"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"id":"spec-crud","serviceId":"service-crud","title":"Overview","content":"# v1"}"##))
        .unwrap();
    let response = app.clone().oneshot(create_specification).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let specification_etag = response.headers().get(ETAG).unwrap().clone();
    let delete_specification = headers(axum::http::Request::delete("/specifications/spec-crud"))
        .header(IF_MATCH, specification_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(delete_specification).await.unwrap().status(), StatusCode::NO_CONTENT);

    let delete_service = headers(axum::http::Request::delete("/services/service-crud"))
        .header(IF_MATCH, service_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(delete_service).await.unwrap().status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn workspace_update_and_delete_are_scoped_to_the_active_workspace() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let create = headers(axum::http::Request::post("/workspaces"))
        .header("content-type", "application/json")
        .header("x-request-id", "workspace-crud")
        .body(Body::from(r#"{"name":"CRUD Workspace"}"#))
        .unwrap();
    let response = app.clone().oneshot(create).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let workspace_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"].as_str().unwrap().to_owned();
    let update = axum::http::Request::put(format!("/workspaces/{workspace_id}"))
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", &workspace_id)
        .header("x-specx-actor-id", "actor-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"name":"Renamed Workspace"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(update).await.unwrap().status(), StatusCode::OK);
    let delete = axum::http::Request::delete(format!("/workspaces/{workspace_id}"))
        .header("x-specx-tenant-id", "tenant-1")
        .header("x-specx-workspace-id", &workspace_id)
        .header("x-specx-actor-id", "actor-1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(delete).await.unwrap().status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn create_requests_are_idempotent_and_service_cursors_bind_filters() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let first = headers(axum::http::Request::post("/groups"))
        .header("idempotency-key", "group-create-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    let response = app.clone().oneshot(first).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let replay = headers(axum::http::Request::post("/groups"))
        .header("idempotency-key", "group-create-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    let replay_response = app.clone().oneshot(replay).await.unwrap();
    assert_eq!(replay_response.status(), StatusCode::OK);
    let changed = headers(axum::http::Request::post("/groups"))
        .header("idempotency-key", "group-create-1")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Changed"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(changed).await.unwrap().status(), StatusCode::CONFLICT);

    for (id, slug) in [("svc-a", "alpha"), ("svc-b", "beta")] {
        let request = headers(axum::http::Request::post("/services"))
            .header("idempotency-key", format!("service-create-{id}"))
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"{slug}","slug":"{slug}"}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
    let first_page = headers(axum::http::Request::get("/services?limit=1")).body(Body::empty()).unwrap();
    let first_body = app.clone().oneshot(first_page).await.unwrap().into_body().collect().await.unwrap().to_bytes();
    let first_page: serde_json::Value = serde_json::from_slice(&first_body).unwrap();
    let cursor = first_page["nextCursor"].as_str().expect("next cursor").to_owned();
    let second_page =
        headers(axum::http::Request::get(format!("/services?limit=1&cursor={cursor}"))).body(Body::empty()).unwrap();
    let second_body = app.clone().oneshot(second_page).await.unwrap().into_body().collect().await.unwrap().to_bytes();
    let second_page: serde_json::Value = serde_json::from_slice(&second_body).unwrap();
    assert_eq!(second_page["items"][0]["slug"], "beta");
    let mismatched = headers(axum::http::Request::get(format!("/services?q=other&limit=1&cursor={cursor}")))
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.oneshot(mismatched).await.unwrap().status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn idempotency_survives_api_restart() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("specx.sqlite");
    let app = router(ApiState::open(&database).unwrap());
    let request = |name: &str| {
        headers(axum::http::Request::post("/groups"))
            .header("idempotency-key", "restart-group-1")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"restart-root","name":"{name}"}}"#)))
            .unwrap()
    };
    assert_eq!(app.clone().oneshot(request("Root")).await.unwrap().status(), StatusCode::OK);
    drop(app);

    let reopened = router(ApiState::open(&database).unwrap());
    let replay = reopened.clone().oneshot(request("Root")).await.unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let changed = reopened.oneshot(request("Changed")).await.unwrap();
    assert_eq!(changed.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn catalog_is_isolated_by_tenant_even_when_workspace_ids_match() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    for (tenant, id, name) in [("tenant-1", "group-1", "Tenant one"), ("tenant-2", "group-2", "Tenant two")] {
        let response = axum::http::Request::post("/groups")
            .header("x-specx-tenant-id", tenant)
            .header("x-specx-workspace-id", "default")
            .header("x-specx-actor-id", "actor")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"id":"{id}","name":"{name}"}}"#)))
            .unwrap();
        assert_eq!(app.clone().oneshot(response).await.unwrap().status(), StatusCode::OK);
    }

    for (tenant, expected) in [("tenant-1", "group-1"), ("tenant-2", "group-2")] {
        let response = axum::http::Request::get("/catalog")
            .header("x-specx-tenant-id", tenant)
            .header("x-specx-workspace-id", "default")
            .header("x-specx-actor-id", "actor")
            .body(Body::empty())
            .unwrap();
        let body = app.clone().oneshot(response).await.unwrap().into_body().collect().await.unwrap().to_bytes();
        let catalog: CatalogResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(catalog.groups.len(), 1);
        assert_eq!(catalog.groups[0].id, expected);
    }
}

#[tokio::test]
async fn created_specification_returns_the_persisted_hash() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let group = headers(axum::http::Request::post("/groups"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(group).await.unwrap().status(), StatusCode::OK);
    let service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-1","groupId":"root","name":"Gateway","slug":"gateway"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(service).await.unwrap().status(), StatusCode::OK);

    let response = headers(axum::http::Request::post("/specifications"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"id":"spec-1","serviceId":"svc-1","title":"Overview","content":"# Gateway"}"##))
        .unwrap();
    let response = app.oneshot(response).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let specification: SpecificationResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(specification.content_hash, "sha256:247e06d69fc8bfaa58b4748636aa85808ac46c42ae0afe48a2fce06a8086628b");
    assert_eq!(specification.version, 1);
    assert_eq!(specification.etag, etag);
}

#[tokio::test]
async fn specification_updates_require_a_fresh_etag_and_append_content_revision() {
    let state = ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap());
    let app = router(state.clone());
    let group = headers(axum::http::Request::post("/groups"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"root","name":"Root"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(group).await.unwrap().status(), StatusCode::OK);
    let service = headers(axum::http::Request::post("/services"))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":"svc-1","groupId":"root","name":"Gateway","slug":"gateway"}"#))
        .unwrap();
    assert_eq!(app.clone().oneshot(service).await.unwrap().status(), StatusCode::OK);
    let create = headers(axum::http::Request::post("/specifications"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"id":"spec-1","serviceId":"svc-1","title":"Overview","content":"# Gateway"}"##))
        .unwrap();
    let created = app.clone().oneshot(create).await.unwrap();
    let original_etag = created.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();

    let missing_precondition = headers(axum::http::Request::put("/specifications/spec-1"))
        .header("content-type", "application/json")
        .body(Body::from(r##"{"title":"Overview","content":"# Updated Gateway"}"##))
        .unwrap();
    let response = app.clone().oneshot(missing_precondition).await.unwrap();
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);

    let update = headers(axum::http::Request::put("/specifications/spec-1"))
        .header("if-match", &original_etag)
        .header("content-type", "application/json")
        .body(Body::from(r##"{"title":"Overview","content":"# Updated Gateway"}"##))
        .unwrap();
    let response = app.clone().oneshot(update).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let updated_etag = response.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    assert_ne!(updated_etag, original_etag);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let updated: SpecificationResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.content, "# Updated Gateway");
    assert_eq!(updated.version, 2);
    assert_eq!(updated.etag, updated_etag);

    let stale_update = headers(axum::http::Request::put("/specifications/spec-1"))
        .header("if-match", original_etag)
        .header("content-type", "application/json")
        .body(Body::from(r##"{"title":"Stale","content":"# Stale"}"##))
        .unwrap();
    let response = app.oneshot(stale_update).await.unwrap();
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let inner = state.inner.lock().await;
    let context =
        WorkspaceContext { tenant_id: "tenant-1".into(), workspace_id: "default".into(), actor_id: "actor-1".into() };
    assert_eq!(inner.store.specification_revision_count(&context, "spec-1").unwrap(), 2);
    assert_eq!(inner.store.audit_event_count(&context).unwrap(), 4);
}

#[tokio::test]
async fn specification_revision_api_lists_reads_and_restores_without_rewriting_history() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    for request in [
        headers(axum::http::Request::post("/groups"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"id":"root","name":"Root"}"#))
            .unwrap(),
        headers(axum::http::Request::post("/services"))
            .header("content-type", "application/json")
            .body(Body::from(r#"{"id":"svc-1","name":"Gateway","slug":"gateway"}"#))
            .unwrap(),
    ] {
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), StatusCode::OK);
    }
    let created = app
        .clone()
        .oneshot(
            headers(axum::http::Request::post("/specifications"))
                .header("content-type", "application/json")
                .body(Body::from(r##"{"id":"spec-1","serviceId":"svc-1","title":"Overview","content":"# v1"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    let etag_v1 = created.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    let updated = app
        .clone()
        .oneshot(
            headers(axum::http::Request::put("/specifications/spec-1"))
                .header("if-match", &etag_v1)
                .header("content-type", "application/json")
                .body(Body::from(r##"{"title":"Overview","content":"# v2"}"##))
                .unwrap(),
        )
        .await
        .unwrap();
    let etag_v2 = updated.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();

    let page = app
        .clone()
        .oneshot(
            headers(axum::http::Request::get("/specifications/spec-1/revisions?limit=1")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let page: serde_json::Value =
        serde_json::from_slice(&page.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page["total"], 2);
    assert_eq!(page["items"][0]["version"], 2);
    assert_eq!(page["nextOffset"], 1);

    let revision = app
        .clone()
        .oneshot(headers(axum::http::Request::get("/specifications/spec-1/revisions/1")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(revision.status(), StatusCode::OK);
    let revision: serde_json::Value =
        serde_json::from_slice(&revision.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(revision["content"], "# v1");

    let restored = app
        .clone()
        .oneshot(
            headers(axum::http::Request::post("/specifications/spec-1/revisions/1/restore"))
                .header("if-match", &etag_v2)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(restored.status(), StatusCode::OK);
    let restored_etag = restored.headers().get(ETAG).unwrap().to_str().unwrap().to_owned();
    assert_ne!(restored_etag, etag_v2);
    let revisions = app
        .clone()
        .oneshot(
            headers(axum::http::Request::get("/specifications/spec-1/revisions?limit=10")).body(Body::empty()).unwrap(),
        )
        .await
        .unwrap();
    let revisions: serde_json::Value =
        serde_json::from_slice(&revisions.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(revisions["total"], 3);
    assert_eq!(revisions["items"][0]["version"], 3);
    assert_eq!(revisions["items"][1]["version"], 2);
    assert_eq!(revisions["items"][2]["version"], 1);
}

#[tokio::test]
async fn domain_errors_have_specific_localized_envelopes_without_debug_text() {
    let cases = [
        (CoreError::EmptyName, 422, "empty_name", "emptyName"),
        (CoreError::EmptySlug, 422, "empty_slug", "emptySlug"),
        (CoreError::EmptyContent, 422, "empty_content", "emptyContent"),
        (CoreError::NotFound("private SQL password=secret"), 404, "not_found", "notFound"),
        (CoreError::ScopeMismatch, 403, "scope_mismatch", "scopeMismatch"),
        (CoreError::InvalidParent, 422, "invalid_parent", "invalidParent"),
        (CoreError::GroupCycle, 409, "group_cycle", "groupCycle"),
        (CoreError::InvalidGroupOrder, 422, "invalid_group_order", "invalidGroupOrder"),
        (CoreError::DuplicateTag, 409, "duplicate_tag", "duplicateTag"),
        (CoreError::DuplicateSlug, 409, "duplicate_slug", "duplicateSlug"),
        (CoreError::DuplicateRepository, 409, "duplicate_repository", "duplicateRepository"),
        (CoreError::InvalidRepository, 422, "invalid_repository", "invalidRepository"),
        (CoreError::ServiceRepositoryExists, 409, "service_repository_exists", "serviceRepositoryExists"),
        (CoreError::DuplicateRefTrack, 409, "duplicate_ref_track", "duplicateRefTrack"),
        (CoreError::InvalidRefTrack, 422, "invalid_ref_track", "invalidRefTrack"),
        (CoreError::HasDependents("private SQL password=secret"), 409, "has_dependents", "hasDependents"),
    ];
    for (error, status, code, key) in cases {
        let response = ApiError::Core(error).into_response();
        assert_eq!(response.status().as_u16(), status);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["version"], 1);
        assert_eq!(body["code"], code);
        assert_eq!(body["messageKey"], format!("apiError.{key}"));
        assert_eq!(body["messageParams"], serde_json::json!({}));
        assert!(body.get("message").is_none());
        assert!(body.get("detail").is_none());
        assert!(!String::from_utf8_lossy(&bytes).contains("secret"));
    }
    let error = PersistenceError::from(std::io::Error::other("private SQL password=secret"));
    let response = ApiError::Persistence(error).into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["messageKey"], "apiError.internal");
    assert!(body.get("message").is_none());
    assert!(!String::from_utf8_lossy(&bytes).contains("secret"));
}

#[tokio::test]
async fn framework_rejections_use_envelopes_and_preserve_status_and_headers() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    let cases = [
        ("POST", "/groups", "application/json", "{private SQL".into(), 400, "invalidRequest"),
        ("POST", "/groups", "application/json", r#"{"name":"A"}"#.into(), 422, "invalidInput"),
        ("POST", "/groups", "text/plain", "private SQL".into(), 415, "unsupportedMediaType"),
        ("POST", "/groups", "application/json", " ".repeat(2 * 1024 * 1024 + 1), 413, "payloadTooLarge"),
        ("GET", "/groups?limit=bad", "application/json", String::new(), 400, "invalidRequest"),
        ("GET", "/groups?limit=201", "application/json", String::new(), 400, "invalidQuery"),
        ("GET", "/specifications/spec-1/revisions/bad", "application/json", String::new(), 400, "invalidRequest"),
        ("PUT", "/catalog", "application/json", String::new(), 405, "methodNotAllowed"),
        ("GET", "/does-not-exist", "application/json", String::new(), 404, "notFound"),
    ];
    for (method, path, content_type, input, status, key) in cases {
        let request = headers(axum::http::Request::builder().method(method).uri(path))
            .header("x-request-id", "error-contract-test")
            .header(CONTENT_TYPE, content_type)
            .body(Body::from(input))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{method} {path}");
        assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
        assert_eq!(response.headers()["x-request-id"], "error-contract-test");
        if status == 405 {
            assert!(response.headers()["allow"].to_str().unwrap().contains("GET"));
        }
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["version"], 1);
        assert_eq!(body["messageKey"], format!("apiError.{key}"));
        assert_eq!(body["messageParams"], serde_json::json!({}));
        assert!(!String::from_utf8_lossy(&bytes).contains("private SQL"));
    }
}

#[tokio::test]
async fn api_errors_use_the_shared_catalog() {
    let cases = [
        (ApiError::Unauthorized, ErrorKind::Unauthorized),
        (ApiError::Forbidden, ErrorKind::Forbidden),
        (ApiError::NotFound, ErrorKind::NotFound),
        (ApiError::OriginRejected, ErrorKind::OriginRejected),
        (ApiError::RateLimited, ErrorKind::RateLimited),
        (ApiError::NotReady, ErrorKind::NotReady),
        (ApiError::InvalidQuery, ErrorKind::InvalidQuery),
        (ApiError::InvalidInput, ErrorKind::InvalidInput),
        (ApiError::LastOwnerRequired, ErrorKind::LastOwnerRequired),
        (ApiError::IdempotencyConflict, ErrorKind::IdempotencyConflict),
        (ApiError::PreconditionRequired, ErrorKind::PreconditionRequired),
        (ApiError::PreconditionFailed, ErrorKind::PreconditionFailed),
    ];
    for (error, kind) in cases {
        let response = error.into_response();
        assert_eq!(response.status(), kind.status());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body, serde_json::to_value(kind.body()).unwrap());
    }
}
