#![cfg(feature = "postgres")]

use axum::body::Body;
use axum::http::{
    Request, StatusCode,
    header::{ETAG, IF_MATCH},
};
use http_body_util::BodyExt;
use specx_api::{ApiState, router};
use std::sync::OnceLock;
use tower::ServiceExt;

fn tenant() -> &'static str {
    static TENANT: OnceLock<String> = OnceLock::new();
    TENANT.get_or_init(|| {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        format!("api-integration-{stamp}")
    })
}

fn request(method: axum::http::Method, path: &str, actor: &str, body: Option<&str>) -> Request<Body> {
    request_in_workspace(method, path, "default", actor, body)
}

fn request_in_workspace(
    method: axum::http::Method,
    path: &str,
    workspace: &str,
    actor: &str,
    body: Option<&str>,
) -> Request<Body> {
    request_in_tenant(tenant(), method, path, workspace, actor, body)
}

fn request_in_tenant(
    tenant_id: &str,
    method: axum::http::Method,
    path: &str,
    workspace: &str,
    actor: &str,
    body: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("x-specx-tenant-id", tenant_id)
        .header("x-specx-workspace-id", workspace)
        .header("x-specx-actor-id", actor);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder.body(body.map_or_else(Body::empty, |value| Body::from(value.to_owned()))).expect("valid request")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_api_enforces_membership_and_specification_cas() {
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let state = ApiState::open_postgres(&url, 4).await.expect("connect to PostgreSQL");
    let app = router::<()>(state.clone());
    let group_id = format!("root-{}", tenant());
    let service_id = format!("service-{}", tenant());
    let specification_id = format!("spec-{}", tenant());

    let ready = app.clone().oneshot(Request::get("/ready").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let ready: serde_json::Value =
        serde_json::from_slice(&ready.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(ready["schemaVersion"], 7);

    let owner_catalog = app.clone().oneshot(request(axum::http::Method::GET, "/catalog", "owner", None)).await.unwrap();
    assert_eq!(owner_catalog.status(), StatusCode::OK);

    let second_catalog =
        app.clone().oneshot(request(axum::http::Method::GET, "/catalog", "second", None)).await.unwrap();
    assert_eq!(second_catalog.status(), StatusCode::FORBIDDEN);

    let grant = app
        .clone()
        .oneshot(request(axum::http::Method::PUT, "/members/second", "owner", Some(r#"{"role":"editor"}"#)))
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::OK);

    let mut group_request =
        request(axum::http::Method::POST, "/groups", "owner", Some(&format!(r#"{{"id":"{group_id}","name":"Root"}}"#)));
    group_request.headers_mut().insert("idempotency-key", "pg-group-replay".parse().unwrap());
    let group = app.clone().oneshot(group_request).await.unwrap();
    let group_status = group.status();
    if group_status != StatusCode::OK {
        eprintln!(
            "group response: {}",
            String::from_utf8_lossy(&group.into_body().collect().await.unwrap().to_bytes())
        );
    }
    assert_eq!(group_status, StatusCode::OK);
    let replay_state = ApiState::open_postgres(&url, 4).await.expect("reopen PostgreSQL API state");
    let replay_app = router::<()>(replay_state);
    let mut replay_request =
        request(axum::http::Method::POST, "/groups", "owner", Some(&format!(r#"{{"id":"{group_id}","name":"Root"}}"#)));
    replay_request.headers_mut().insert("idempotency-key", "pg-group-replay".parse().unwrap());
    assert_eq!(replay_app.clone().oneshot(replay_request).await.unwrap().status(), StatusCode::OK);
    let mut changed_request = request(
        axum::http::Method::POST,
        "/groups",
        "owner",
        Some(&format!(r#"{{"id":"{group_id}","name":"Changed"}}"#)),
    );
    changed_request.headers_mut().insert("idempotency-key", "pg-group-replay".parse().unwrap());
    assert_eq!(replay_app.oneshot(changed_request).await.unwrap().status(), StatusCode::CONFLICT);
    let service = app
        .clone()
        .oneshot(request(
            axum::http::Method::POST,
            "/services",
            "owner",
            Some(&format!(
                r#"{{"id":"{service_id}","groupId":"{group_id}","name":"Gateway","slug":"gateway-{service_id}"}}"#
            )),
        ))
        .await
        .unwrap();
    assert_eq!(service.status(), StatusCode::OK);
    let created = app
        .clone()
        .oneshot(request(
            axum::http::Method::POST,
            "/specifications",
            "owner",
            Some(&format!(
                r##"{{"id":"{specification_id}","serviceId":"{service_id}","title":"Overview","content":"# v1"}}"##
            )),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let etag = created.headers().get(ETAG).expect("ETag").to_owned();

    let editor_catalog =
        app.clone().oneshot(request(axum::http::Method::GET, "/catalog", "second", None)).await.unwrap();
    assert_eq!(editor_catalog.status(), StatusCode::OK);

    let update = Request::builder()
        .method(axum::http::Method::PUT)
        .uri(format!("/specifications/{specification_id}"))
        .header("x-specx-tenant-id", tenant())
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "second")
        .header(IF_MATCH, etag)
        .header("content-type", "application/json")
        .body(Body::from(r##"{"title":"Overview","content":"# v2"}"##))
        .unwrap();
    let update = app.clone().oneshot(update).await.unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let updated_specification_etag = update.headers().get(ETAG).expect("updated specification ETag").to_owned();

    let delete_specification = Request::builder()
        .method(axum::http::Method::DELETE)
        .uri(format!("/specifications/{specification_id}"))
        .header("x-specx-tenant-id", tenant())
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "owner")
        .header(IF_MATCH, updated_specification_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(delete_specification).await.unwrap().status(), StatusCode::NO_CONTENT);

    let services = app.clone().oneshot(request(axum::http::Method::GET, "/services", "owner", None)).await.unwrap();
    assert_eq!(services.status(), StatusCode::OK);
    let services: serde_json::Value =
        serde_json::from_slice(&services.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let service_etag = services["items"][0]["etag"].as_str().expect("service ETag").to_owned();
    let delete_service = Request::builder()
        .method(axum::http::Method::DELETE)
        .uri(format!("/services/{service_id}"))
        .header("x-specx-tenant-id", tenant())
        .header("x-specx-workspace-id", "default")
        .header("x-specx-actor-id", "owner")
        .header(IF_MATCH, service_etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(delete_service).await.unwrap().status(), StatusCode::NO_CONTENT);
    let delete_group = app
        .clone()
        .oneshot(request(axum::http::Method::DELETE, &format!("/groups/{group_id}"), "owner", None))
        .await
        .unwrap();
    assert_eq!(delete_group.status(), StatusCode::NO_CONTENT);

    let invitation = app
        .clone()
        .oneshot(request(
            axum::http::Method::POST,
            "/invitations",
            "owner",
            Some(r#"{"email":"invitee@example.com","role":"viewer"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(invitation.status(), StatusCode::CREATED);
    let invitation_body: serde_json::Value =
        serde_json::from_slice(&invitation.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let invitation_token = invitation_body["token"].as_str().expect("one-time bearer token").to_owned();
    assert!(invitation_body.get("tokenHash").is_none());

    let invitations =
        app.clone().oneshot(request(axum::http::Method::GET, "/invitations", "owner", None)).await.unwrap();
    assert_eq!(invitations.status(), StatusCode::OK);
    let invitations: serde_json::Value =
        serde_json::from_slice(&invitations.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(invitations.as_array().unwrap().len(), 1);
    assert!(invitations[0].get("token").is_none());
    assert!(invitations[0].get("tokenHash").is_none());

    let revoke_invitation = app
        .clone()
        .oneshot(request(
            axum::http::Method::POST,
            "/invitations",
            "owner",
            Some(r#"{"email":"revoke@example.com","role":"viewer"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(revoke_invitation.status(), StatusCode::CREATED);
    let revoke_body: serde_json::Value =
        serde_json::from_slice(&revoke_invitation.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let revoke_id = revoke_body["id"].as_str().expect("invitation id");
    let revoke_token = revoke_body["token"].as_str().expect("invitation token");
    let revoke = app
        .clone()
        .oneshot(request(axum::http::Method::DELETE, &format!("/invitations/{revoke_id}"), "owner", None))
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);
    let revoked_accept = Request::builder()
        .method(axum::http::Method::POST)
        .uri("/invitations/accept")
        .header("x-specx-tenant-id", tenant())
        .header("x-specx-actor-id", "revoked-invitee")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": revoke_token}).to_string()))
        .unwrap();
    assert_eq!(app.clone().oneshot(revoked_accept).await.unwrap().status(), StatusCode::NOT_FOUND);

    let accept = Request::builder()
        .method(axum::http::Method::POST)
        .uri("/invitations/accept")
        .header("x-specx-tenant-id", tenant())
        .header("x-specx-actor-id", "invitee")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": invitation_token}).to_string()))
        .unwrap();
    let accepted = app.clone().oneshot(accept).await.unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted: serde_json::Value =
        serde_json::from_slice(&accepted.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(accepted["role"], "viewer");

    let invitee_access =
        app.clone().oneshot(request(axum::http::Method::GET, "/access", "invitee", None)).await.unwrap();
    assert_eq!(invitee_access.status(), StatusCode::OK);
    let replay = Request::builder()
        .method(axum::http::Method::POST)
        .uri("/invitations/accept")
        .header("x-specx-tenant-id", tenant())
        .header("x-specx-actor-id", "invitee")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::json!({"token": invitation_token}).to_string()))
        .unwrap();
    assert_eq!(app.clone().oneshot(replay).await.unwrap().status(), StatusCode::NOT_FOUND);

    let revoke =
        app.clone().oneshot(request(axum::http::Method::DELETE, "/members/second", "owner", None)).await.unwrap();
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);
    let revoked_catalog = app.oneshot(request(axum::http::Method::GET, "/catalog", "second", None)).await.unwrap();
    assert_eq!(revoked_catalog.status(), StatusCode::FORBIDDEN);

    let _ = created.into_body().collect().await;

    #[cfg(unix)]
    {
        let backup_root = tempfile::tempdir().expect("backup root");
        let dump_script = backup_root.path().join("fake-pg-dump.sh");
        let restore_script = backup_root.path().join("fake-pg-restore.sh");
        std::fs::write(&dump_script, "#!/bin/sh\nout=\"\"\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--file\" ]; then out=\"$2\"; shift 2; else shift; fi\ndone\nprintf 'api-backup' > \"$out\"\n").unwrap();
        std::fs::write(&restore_script, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        for path in [&dump_script, &restore_script] {
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(path, permissions).unwrap();
        }
        unsafe {
            std::env::set_var("SPECX_PG_DUMP_BIN", &dump_script);
            std::env::set_var("SPECX_PG_RESTORE_BIN", &restore_script);
        }

        let backup_state =
            ApiState::open_postgres_with_backup_root(&url, 4, backup_root.path()).await.expect("backup API state");
        let backup_app = router::<()>(backup_state);
        let system_bootstrap = backup_app
            .clone()
            .oneshot(request_in_tenant("specx-web", axum::http::Method::GET, "/catalog", "default", "owner", None))
            .await
            .unwrap();
        assert_eq!(system_bootstrap.status(), StatusCode::OK);
        let non_system_backup = backup_app
            .clone()
            .oneshot(request_in_tenant(tenant(), axum::http::Method::GET, "/backups", "default", "owner", None))
            .await
            .unwrap();
        assert_eq!(non_system_backup.status(), StatusCode::FORBIDDEN);
        let non_default_backup = backup_app
            .clone()
            .oneshot(request_in_tenant("specx-web", axum::http::Method::GET, "/backups", "secondary", "owner", None))
            .await
            .unwrap();
        assert_eq!(non_default_backup.status(), StatusCode::FORBIDDEN);
        let created_backup = backup_app
            .clone()
            .oneshot(request_in_tenant("specx-web", axum::http::Method::POST, "/backups", "default", "owner", None))
            .await
            .unwrap();
        assert_eq!(created_backup.status(), StatusCode::OK);
        let backup_body: serde_json::Value =
            serde_json::from_slice(&created_backup.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let file_name = backup_body["fileName"].as_str().expect("backup file name").to_owned();
        assert!(file_name.ends_with(".dump"));
        assert_eq!(backup_body["sha256"].as_str().unwrap().len(), 64);

        let listed = backup_app
            .clone()
            .oneshot(request_in_tenant("specx-web", axum::http::Method::GET, "/backups", "default", "owner", None))
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let listed_body: serde_json::Value =
            serde_json::from_slice(&listed.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(listed_body.as_array().unwrap().len(), 1);

        let restored = backup_app
            .oneshot(request_in_tenant(
                "specx-web",
                axum::http::Method::POST,
                &format!("/backups/{file_name}/restore"),
                "default",
                "owner",
                None,
            ))
            .await
            .unwrap();
        assert_eq!(restored.status(), StatusCode::OK);
        unsafe {
            std::env::remove_var("SPECX_PG_DUMP_BIN");
            std::env::remove_var("SPECX_PG_RESTORE_BIN");
        }
    }
}
