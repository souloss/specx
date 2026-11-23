use super::*;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    actor: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(actor) = actor {
        request = request
            .header("x-specx-tenant-id", "tenant-1")
            .header("x-specx-workspace-id", "default")
            .header("x-specx-actor-id", actor);
    }
    let request = if let Some(body) = body {
        request.header("content-type", "application/json").body(Body::from(body.to_string()))
    } else {
        request.body(Body::empty())
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn publication_is_an_authorized_immutable_revocable_snapshot() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    assert_eq!(
        call(
            &app,
            "POST",
            "/services",
            Some("actor-1"),
            Some(serde_json::json!({"id":"published-service","name":"Published","slug":"published"}))
        )
        .await
        .0,
        StatusCode::OK
    );
    let source = serde_json::json!({"openapi": "3.1.0", "info": {"title": "Public API", "version": "1"}, "servers": [{"url": "https://internal.test"}], "paths": {"/pets": {"servers": [{"url": "https://internal.test/path"}], "get": {"servers": [{"url": "https://internal.test/operation"}], "responses": {"200": {"description": "OK", "content": {"application/json": {"schema": {"type": "object", "properties": {"servers": {"type": "array", "items": {"type": "string"}}}}}}}}}}}});
    assert_eq!(call(&app, "POST", "/specifications", Some("actor-1"), Some(serde_json::json!({"id":"published-spec","serviceId":"published-service","title":"Public API","kind":"openapi","content":source.to_string()}))).await.0, StatusCode::OK);
    let base = "/specifications/published-spec";
    let (_, preview) = call(&app, "GET", &format!("{base}/publication-preview/1"), Some("actor-1"), None).await;
    let snapshot: serde_json::Value = serde_json::from_str(preview["content"].as_str().unwrap()).unwrap();
    let mut expected = source.clone();
    expected.as_object_mut().unwrap().remove("servers");
    expected["paths"]["/pets"].as_object_mut().unwrap().remove("servers");
    expected["paths"]["/pets"]["get"].as_object_mut().unwrap().remove("servers");
    assert_eq!(snapshot, expected);
    let mut input = serde_json::json!({"title":"Shared API","description":"Stable public reference","theme":"dark","showDownload":false,"version":1,"digest":"unreviewed"});
    assert_eq!(
        call(&app, "POST", &format!("{base}/publications"), Some("actor-1"), Some(input.clone())).await.0,
        StatusCode::PRECONDITION_FAILED
    );
    input["digest"] = preview["digest"].clone();
    let (status, publication) =
        call(&app, "POST", &format!("{base}/publications"), Some("actor-1"), Some(input.clone())).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        call(&app, "POST", &format!("{base}/publications"), Some("actor-1"), Some(input)).await.0,
        StatusCode::CONFLICT
    );
    let public = format!("/public/openapi/{}", publication["id"].as_str().unwrap());
    let (status, published) = call(&app, "GET", &public, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(published["content"], preview["content"]);
    assert_eq!(published["publication"]["title"], "Shared API");
    assert_eq!(published["publication"]["description"], "Stable public reference");
    assert_eq!(published["publication"]["theme"], "dark");
    assert_eq!(published["publication"]["showDownload"], false);
    assert_eq!(published["publication"]["allowTryIt"], true);
    let (_, list) = call(&app, "GET", &format!("{base}/publications?limit=1"), Some("actor-1"), None).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert!(list["nextOffset"].is_null());
    assert_eq!(
        call(&app, "GET", &format!("{base}/publications?limit=201"), Some("actor-1"), None).await.0,
        StatusCode::BAD_REQUEST
    );
    for (actor, role) in [("reader", "viewer"), ("writer", "editor")] {
        assert_eq!(
            call(&app, "PUT", &format!("/members/{actor}"), Some("actor-1"), Some(serde_json::json!({"role":role})))
                .await
                .0,
            StatusCode::OK
        );
        for method in ["GET", "POST"] {
            assert_eq!(
                call(
                    &app,
                    method,
                    &format!("{base}/publications"),
                    Some(actor),
                    Some(serde_json::json!({"title":"No","version":1,"digest":preview["digest"]}))
                )
                .await
                .0,
                StatusCode::FORBIDDEN
            );
        }
    }
    assert_eq!(
        call(&app, "GET", &format!("{base}/publication-preview/1"), None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let revoke = format!("{base}/publications/{}", publication["id"].as_str().unwrap());
    assert_eq!(call(&app, "DELETE", &revoke, Some("writer"), None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "DELETE", &revoke, Some("actor-1"), None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "GET", &public, None, None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(&app, "GET", &format!("{base}/revisions/1"), Some("actor-1"), None).await.0, StatusCode::OK);

    // A replacement publication carries its own capabilities. Revoking one
    // link cannot make an older link work again or change the source revision.
    let (status, restricted) = call(
        &app,
        "POST",
        &format!("{base}/publications"),
        Some("actor-1"),
        Some(serde_json::json!({
            "title": "Reference only", "version": 1, "digest": preview["digest"],
            "allowTryIt": false, "showDownload": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(restricted["id"], publication["id"]);
    let restricted_url = format!("/public/openapi/{}", restricted["id"].as_str().unwrap());
    let (_, document) = call(&app, "GET", &restricted_url, None, None).await;
    assert_eq!(document["publication"]["allowTryIt"], false);
    assert_eq!(document["publication"]["showDownload"], false);
    assert_eq!(call(&app, "GET", &public, None, None).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn publication_carries_only_explicit_validated_portable_environments() {
    let app = router(ApiState::new(Core::default(), SqliteStore::open_in_memory().unwrap()));
    call(
        &app,
        "POST",
        "/services",
        Some("actor-1"),
        Some(serde_json::json!({"id":"env-service","name":"Demo","slug":"demo"})),
    )
    .await;
    let source = serde_json::json!({"openapi":"3.1.0","info":{"title":"Demo","version":"1"},"servers":[{"url":"https://private.invalid"}],"x-specx-public-environments":[{"name":"Unreviewed","baseUrl":"https://private.invalid","variables":[]}],"paths":{"/posts":{"get":{"responses":{"200":{"description":"OK"}}}}}});
    call(&app, "POST", "/specifications", Some("actor-1"), Some(serde_json::json!({"id":"env-spec","serviceId":"env-service","title":"Demo","kind":"openapi","content":source.to_string()}))).await;
    let (_, preview) = call(&app, "GET", "/specifications/env-spec/publication-preview/1", Some("actor-1"), None).await;
    assert!(!preview["content"].as_str().unwrap().contains("private.invalid"));
    let environment = serde_json::json!({"name":"Public Demo","baseUrl":"https://{{host}}","variables":[{"key":"host","value":"jsonplaceholder.typicode.com"},{"key":"userId","value":"1"}]});
    let input = serde_json::json!({"title":"Demo","version":1,"digest":preview["digest"],"environments":[environment]});
    for url in [
        "/api",
        "javascript:alert(1)",
        "https://user:password@example.test",
        "https://example.test?token=x",
        "https://example.test/#fragment",
        "https://{{missing}}",
        "https://example.test/\\evil",
        "https://",
        "https://example.test/ with-space",
    ] {
        let mut invalid = input.clone();
        invalid["environments"][0]["baseUrl"] = url.into();
        assert_eq!(
            call(&app, "POST", "/specifications/env-spec/publications", Some("actor-1"), Some(invalid)).await.0,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{url}"
        );
    }
    for environments in [
        serde_json::json!([environment, environment]),
        serde_json::json!([{"name":"Demo","baseUrl":"https://example.test","variables":[{"key":"host","value":"a"},{"key":"host","value":"b"}]}]),
        serde_json::json!([{"name":"Demo","baseUrl":"https://example.test","variables":[],"secretId":"hidden"}]),
    ] {
        let mut invalid = input.clone();
        invalid["environments"] = environments;
        assert_eq!(
            call(&app, "POST", "/specifications/env-spec/publications", Some("actor-1"), Some(invalid)).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let (status, publication) =
        call(&app, "POST", "/specifications/env-spec/publications", Some("actor-1"), Some(input)).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, published) =
        call(&app, "GET", &format!("/public/openapi/{}", publication["id"].as_str().unwrap()), None, None).await;
    let snapshot: serde_json::Value = serde_json::from_str(published["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        snapshot["servers"],
        serde_json::json!([{"url":"https://jsonplaceholder.typicode.com","description":"Public Demo"}])
    );
    assert_eq!(snapshot["x-specx-public-environments"], serde_json::json!([environment]));
    let (_, revision) = call(&app, "GET", "/specifications/env-spec/revisions/1", Some("actor-1"), None).await;
    assert_eq!(serde_json::from_str::<serde_json::Value>(revision["content"].as_str().unwrap()).unwrap(), source);
}
