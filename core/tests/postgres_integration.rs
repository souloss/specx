#![cfg(feature = "postgres")]

use chrono::Utc;
use specx_core::persistence::{AuditEvent, WebSession};
use specx_core::postgres::PostgresStore;
use specx_core::{Core, Service, Specification, SpecificationKind, WorkspaceContext};

static POSTGRES_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn postgres_migrations_serialize_across_clients() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let first = PostgresStore::connect(&url, 2).await.expect("connect first migration client");
    let second = PostgresStore::connect(&url, 2).await.expect("connect second migration client");
    let (first_result, second_result) = tokio::join!(first.migrate(), second.migrate());
    first_result.expect("first concurrent migration");
    second_result.expect("second concurrent migration");
    assert!(first.ready().await.expect("first migration readiness"));
    assert!(second.ready().await.expect("second migration readiness"));
}

#[tokio::test]
async fn postgres_web_sessions_revoke_across_clients() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let first = PostgresStore::connect(&url, 2).await.expect("connect first session client");
    let second = PostgresStore::connect(&url, 2).await.expect("connect second session client");
    let token_hash = "sha256:session-integration".to_owned();
    let session = WebSession {
        token_hash: token_hash.clone(),
        actor_id: "actor-session".into(),
        expires_at: "2026-10-25T01:00:00Z".into(),
        revoked_at: None,
        created_at: "2026-10-25T00:00:00Z".into(),
    };
    first.create_web_session(&session).await.expect("create web session");
    assert!(second.web_session_is_active(&token_hash, "2026-10-25T00:30:00Z").await.unwrap());
    assert!(second.revoke_web_session(&token_hash, "2026-10-25T00:40:00Z").await.unwrap());
    assert!(!first.web_session_is_active(&token_hash, "2026-10-25T00:41:00Z").await.unwrap());
}

#[tokio::test]
async fn postgres_backup_retention_lock_serializes_distinct_clients() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let first = PostgresStore::connect(&url, 2).await.expect("connect first PostgreSQL client");
    let second = PostgresStore::connect(&url, 2).await.expect("connect second PostgreSQL client");
    let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let peak = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let key = (1_827_364_501, -1_209_384_756);

    let first_active = active.clone();
    let first_peak = peak.clone();
    let first_operation = first.with_backup_retention_lock(key, move || async move {
        let current = first_active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        first_peak.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        first_active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        Ok::<(), specx_core::persistence::PersistenceError>(())
    });
    let second_active = active.clone();
    let second_peak = peak.clone();
    let second_operation = second.with_backup_retention_lock(key, move || async move {
        let current = second_active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        second_peak.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        second_active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        Ok::<(), specx_core::persistence::PersistenceError>(())
    });
    let (first_result, second_result) = tokio::join!(first_operation, second_operation);
    first_result.expect("first retention operation");
    second_result.expect("second retention operation");
    assert_eq!(peak.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn postgres_managed_backup_job_rotates_hashes_and_rolls_back_failures() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let directory = tempfile::tempdir().expect("backup directory");
    let dump_script = directory.path().join("fake-pg-dump.sh");
    let restore_script = directory.path().join("fake-pg-restore.sh");
    let marker = directory.path().join("restore.marker");
    std::fs::write(&dump_script, "#!/bin/sh\nout=\"\"\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--file\" ]; then out=\"$2\"; shift 2; else shift; fi\ndone\nprintf 'fake-postgres-snapshot' > \"$out\"\n").unwrap();
    std::fs::write(&restore_script, "#!/bin/sh\nsrc=\"\"\nfor arg in \"$@\"; do src=\"$arg\"; done\ngrep -q fake-postgres-snapshot \"$src\" || exit 9\nprintf restored > \"$SPECX_TEST_RESTORE_MARKER\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    for path in [&dump_script, &restore_script] {
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(path, permissions).unwrap();
    }
    unsafe {
        std::env::set_var("SPECX_PG_DUMP_BIN", &dump_script);
        std::env::set_var("SPECX_PG_RESTORE_BIN", &restore_script);
        std::env::set_var("SPECX_TEST_RESTORE_MARKER", &marker);
        std::env::set_var("SPECX_BACKUP_COMMAND_TIMEOUT_SECS", "5");
        std::env::set_var("SPECX_BACKUP_ENCRYPTION_KEYS", "old=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
        std::env::set_var("SPECX_BACKUP_ENCRYPTION_KEY_ID", "old");
    }
    let store = PostgresStore::connect(&url, 4).await.expect("connect to PostgreSQL");
    let first = store.create_backup(directory.path().to_path_buf(), "managed", 3).await.expect("first dump");
    unsafe {
        // A deployment can publish the new key while retaining the old one
        // for restores, then select it for newly created snapshots.
        std::env::set_var(
            "SPECX_BACKUP_ENCRYPTION_KEYS",
            "old=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=,new=//////////////////////////////////////////8=",
        );
        std::env::set_var("SPECX_BACKUP_ENCRYPTION_KEY_ID", "new");
    }
    let second = store.create_backup(directory.path().to_path_buf(), "managed", 3).await.expect("second dump");
    let third = store.create_backup(directory.path().to_path_buf(), "managed", 3).await.expect("third dump");
    assert_ne!(first.sha256, second.sha256);
    assert_eq!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "dump")
                && std::fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink()))
            .count(),
        3
    );
    assert!(first.path.exists());
    assert!(third.path.exists());
    assert!(third.bytes > b"fake-postgres-snapshot".len() as u64);
    assert!(!std::fs::read(&third.path).unwrap().starts_with(b"fake-postgres-snapshot"));
    assert!(std::fs::read(&third.path).unwrap().starts_with(b"SPECXPG2"));
    store.restore_backup(first.path.clone()).await.expect("restore snapshot with retained old key");
    store.restore_backup(third.path.clone()).await.expect("restore dump");
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "restored");
    let fourth = store.create_backup(directory.path().to_path_buf(), "managed", 3).await.expect("fourth dump");
    assert!(!first.path.exists());
    assert!(fourth.path.exists());
    assert_eq!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "dump")
                && std::fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink()))
            .count(),
        3
    );
    let symlink = directory.path().join("managed-link.dump");
    std::os::unix::fs::symlink(&third.path, &symlink).unwrap();
    assert!(store.restore_backup(symlink).await.is_err());

    unsafe {
        std::env::set_var("SPECX_PG_DUMP_BIN", "/bin/false");
    }
    let failed = store.create_backup(directory.path().to_path_buf(), "managed", 2).await;
    assert!(failed.is_err());
    assert_eq!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "dump")
                && std::fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink()))
            .count(),
        3
    );
    unsafe {
        std::env::remove_var("SPECX_PG_DUMP_BIN");
        std::env::remove_var("SPECX_PG_RESTORE_BIN");
        std::env::remove_var("SPECX_TEST_RESTORE_MARKER");
        std::env::remove_var("SPECX_BACKUP_COMMAND_TIMEOUT_SECS");
        std::env::remove_var("SPECX_BACKUP_ENCRYPTION_KEYS");
        std::env::remove_var("SPECX_BACKUP_ENCRYPTION_KEY_ID");
    }
}

#[tokio::test]
async fn postgres_round_trip_is_scoped_and_transactional() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let store = PostgresStore::connect(&url, 4).await.expect("connect to PostgreSQL");
    assert!(store.ready().await.expect("schema is ready"));

    let suffix = format!("{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
    let context = WorkspaceContext {
        tenant_id: format!("tenant-{suffix}"),
        // The API's first-login bootstrap is intentionally limited to the
        // default workspace.  Use that same production bootstrap path here;
        // non-default workspaces must be provisioned by an existing owner.
        workspace_id: "default".into(),
        actor_id: "owner".into(),
    };
    let mut core = Core::default();
    let group_id = format!("root-{suffix}");
    core.create_group(&context, group_id.clone(), "Root".into(), None).unwrap();
    core.create_service(
        &context,
        Service {
            id: format!("svc-{suffix}"),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            group_id: Some(group_id),
            name: "Gateway".into(),
            slug: "gateway".into(),
            description: "edge".into(),
            tag_ids: vec![],
            updated_at: Utc::now().to_rfc3339(),
            archived: false,
        },
    )
    .unwrap();
    core.create_markdown_specification(
        &context,
        Specification {
            id: format!("spec-{suffix}"),
            tenant_id: context.tenant_id.clone(),
            workspace_id: context.workspace_id.clone(),
            service_id: format!("svc-{suffix}"),
            title: "README".into(),
            kind: SpecificationKind::Markdown,
            content: "# Gateway".into(),
            version: 1,
            content_hash: String::new(),
            updated_at: Utc::now().to_rfc3339(),
        },
    )
    .unwrap();

    let event = AuditEvent {
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        actor_id: context.actor_id.clone(),
        request_id: "request-1".into(),
        action: "seed".into(),
        resource_type: "catalog".into(),
        resource_id: "workspace".into(),
        created_at: Utc::now().to_rfc3339(),
    };
    store.persist_core_with_audit(&core, &context, Some(&event)).await.unwrap();
    store.persist_core(&core, &context).await.unwrap();

    let loaded = store.load_core(&context).await.unwrap();
    assert_eq!(loaded.list_services(&context).len(), 1);
    assert_eq!(loaded.list_specifications(&context).len(), 1);
    assert_eq!(store.audit_event_count(&context).await.unwrap(), 1);
    assert!(store.authorize_workspace(&context).await.unwrap());
    let peer_store = PostgresStore::connect(&url, 2).await.expect("peer PostgreSQL store");
    let expected_service = loaded.list_services(&context)[0].clone();
    let peer_expected_service = peer_store.load_core(&context).await.unwrap().list_services(&context)[0].clone();
    let mut updated_service = expected_service.clone();
    updated_service.description = "updated once".into();
    updated_service.updated_at = Utc::now().to_rfc3339();
    let mut stale_service = peer_expected_service.clone();
    stale_service.description = "stale overwrite".into();
    stale_service.updated_at = Utc::now().to_rfc3339();
    let service_audit = AuditEvent {
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        actor_id: context.actor_id.clone(),
        request_id: format!("service-cas-{suffix}"),
        action: "update".into(),
        resource_type: "service".into(),
        resource_id: expected_service.id.clone(),
        created_at: Utc::now().to_rfc3339(),
    };
    assert!(
        store
            .update_service_if_current_with_audit(&context, &expected_service, &updated_service, Some(&service_audit))
            .await
            .unwrap()
    );
    assert!(
        !peer_store
            .update_service_if_current_with_audit(
                &context,
                &peer_expected_service,
                &stale_service,
                Some(&service_audit)
            )
            .await
            .unwrap()
    );
    let persisted_service = store.load_core(&context).await.unwrap().list_services(&context)[0].clone();
    assert_eq!(persisted_service.description, "updated once");
    assert_eq!(store.audit_event_count(&context).await.unwrap(), 2);
    let member_actor = format!("member-{suffix}");
    let member_audit = AuditEvent {
        tenant_id: context.tenant_id.clone(),
        workspace_id: context.workspace_id.clone(),
        actor_id: context.actor_id.clone(),
        request_id: "request-member".into(),
        action: "grant".into(),
        resource_type: "workspace_member".into(),
        resource_id: member_actor.clone(),
        created_at: Utc::now().to_rfc3339(),
    };
    assert!(store.set_workspace_member_role(&context, &member_actor, "editor", &member_audit).await.unwrap());
    assert_eq!(store.workspace_member_role(&context, &member_actor).await.unwrap().as_deref(), Some("editor"));
    assert_eq!(store.workspace_members(&context).await.unwrap().len(), 2);
    let loaded_spec = store.load_core(&context).await.unwrap().list_specifications(&context)[0].clone();
    let mut changed_spec = loaded_spec.clone();
    changed_spec.content = "# Gateway v2".into();
    changed_spec.updated_at = Utc::now().to_rfc3339();
    assert!(
        store.update_specification_if_current_with_audit(&context, &loaded_spec, &changed_spec, None).await.unwrap()
    );
    assert!(
        !store.update_specification_if_current_with_audit(&context, &loaded_spec, &changed_spec, None).await.unwrap()
    );
    assert_eq!(store.specification_revision_count(&context, &changed_spec.id).await.unwrap(), 2);
    let revisions = store.specification_revisions(&context, &changed_spec.id, 10, 0).await.unwrap();
    assert_eq!(revisions.iter().map(|revision| revision.version).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(revisions[0].content, "# Gateway v2");
    assert_eq!(
        store.specification_revision(&context, &changed_spec.id, 1).await.unwrap().unwrap().content,
        "# Gateway"
    );
    let other_workspace = WorkspaceContext { workspace_id: format!("other-{suffix}"), ..context.clone() };
    assert!(store.specification_revision(&other_workspace, &changed_spec.id, 1).await.unwrap().is_none());
    let revoke_audit = AuditEvent {
        action: "revoke".into(),
        request_id: "request-revoke".into(),
        resource_id: member_actor.clone(),
        ..member_audit
    };
    assert!(store.revoke_workspace_member_with_audit(&context, &member_actor, &revoke_audit).await.unwrap());
    assert!(!store.authorize_workspace(&WorkspaceContext { actor_id: member_actor, ..context.clone() }).await.unwrap());
    assert!(store.consume_rate_limit(&context, 100, 60, 1).await.unwrap());
    assert!(!store.consume_rate_limit(&context, 120, 60, 1).await.unwrap());
    let peer = PostgresStore::connect(&url, 2).await.expect("peer PostgreSQL store");
    let shared_limit_context = WorkspaceContext { actor_id: format!("rate-peer-{suffix}"), ..context.clone() };
    assert!(store.consume_rate_limit(&shared_limit_context, 100, 60, 1).await.unwrap());
    assert!(!peer.consume_rate_limit(&shared_limit_context, 120, 60, 1).await.unwrap());
}

#[tokio::test]
async fn postgres_purges_only_expired_pending_invitations() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let store = PostgresStore::connect(&url, 2).await.expect("connect to PostgreSQL");
    let suffix = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let tenant = format!("invitation-cleanup-{suffix}");
    let ids = [
        format!("expired-pending-{suffix}"),
        format!("expired-accepted-{suffix}"),
        format!("expired-revoked-{suffix}"),
        format!("unexpired-pending-{suffix}"),
    ];
    let client = store.pool().get().await.expect("PostgreSQL client");
    let has_expiry_index = client
        .query_opt(
            "SELECT 1 FROM pg_indexes WHERE schemaname=current_schema() AND indexname='idx_workspace_invitations_expires_at'",
            &[],
        )
        .await
        .expect("query invitation expiry index");
    assert!(has_expiry_index.is_some());
    for (index, id) in ids.iter().enumerate() {
        let (expires_at, accepted_at, revoked_at) = match index {
            1 => ("2020-01-01T00:00:00Z", Some("2025-01-01T00:00:00Z"), None),
            2 => ("2020-01-01T00:00:00Z", None, Some("2025-01-01T00:00:00Z")),
            3 => ("2099-01-01T00:00:00Z", None, None),
            _ => ("2020-01-01T00:00:00Z", None, None),
        };
        client
            .execute(
                "INSERT INTO workspace_invitations(id,tenant_id,workspace_id,email,role,token_hash,expires_at,accepted_at,revoked_at,created_by,created_at)
                 VALUES($1,$2,'default',$3,'viewer',$4,$5,$6,$7,'owner','2025-01-01T00:00:00Z')",
                &[
                    id,
                    &tenant,
                    &format!("{id}@example.com"),
                    &format!("hash:{id}"),
                    &expires_at,
                    &accepted_at,
                    &revoked_at,
                ],
            )
            .await
            .expect("insert invitation cleanup fixture");
    }

    assert_eq!(store.purge_expired_workspace_invitations("2026-01-01T00:00:00Z").await.unwrap(), 1);
    let remaining = client
        .query("SELECT id FROM workspace_invitations WHERE tenant_id=$1 ORDER BY id", &[&tenant])
        .await
        .expect("query remaining invitations")
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert_eq!(remaining, vec![ids[1].clone(), ids[2].clone(), ids[3].clone()]);
}

#[tokio::test]
async fn postgres_tls_handshake_and_client_rotation_reload() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else {
        return;
    };
    let Ok(rotated_cert) = std::env::var("SPECX_POSTGRES_ROTATED_CLIENT_CERT") else {
        return;
    };
    let Ok(rotated_key) = std::env::var("SPECX_POSTGRES_ROTATED_CLIENT_KEY") else {
        return;
    };
    let mut store = PostgresStore::connect(&url, 2).await.expect("initial TLS connection");
    assert!(store.ready().await.expect("initial TLS readiness"));
    unsafe {
        std::env::set_var("SPECX_POSTGRES_CLIENT_CERT", rotated_cert);
        std::env::set_var("SPECX_POSTGRES_CLIENT_KEY", rotated_key);
    }
    store.reload_credentials(2).await.expect("rotated TLS connection");
    assert!(store.ready().await.expect("rotated TLS readiness"));
    unsafe {
        std::env::remove_var("SPECX_POSTGRES_ROTATED_CLIENT_CERT");
        std::env::remove_var("SPECX_POSTGRES_ROTATED_CLIENT_KEY");
    }
}

#[tokio::test]
async fn postgres_tls_policy_rejects_explicit_plaintext_when_required() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    unsafe {
        std::env::set_var("SPECX_POSTGRES_REQUIRE_TLS", "1");
    }
    let result = PostgresStore::connect("postgres://postgres:specx@127.0.0.1:55432/specx?sslmode=disable", 1).await;
    assert!(result.is_err());
    unsafe {
        std::env::remove_var("SPECX_POSTGRES_REQUIRE_TLS");
    }
}

#[tokio::test]
async fn postgres_migration_failure_rolls_back_the_entire_transaction() {
    let _guard = POSTGRES_TEST_LOCK.lock().await;
    let Ok(url) = std::env::var("SPECX_TEST_POSTGRES_URL") else { return };
    let store = PostgresStore::connect(&url, 2).await.expect("connect to PostgreSQL");
    let mut client = store.pool().get().await.expect("pool client");
    let transaction = client.transaction().await.expect("transaction");
    let result = transaction
        .batch_execute(
            "CREATE TABLE migration_rollback_probe(id INTEGER); SELECT definitely_missing_migration_function();",
        )
        .await;
    assert!(result.is_err());
    transaction.rollback().await.expect("rollback migration transaction");
    let exists = client
        .query_opt("SELECT 1 FROM information_schema.tables WHERE table_name='migration_rollback_probe'", &[])
        .await
        .expect("probe query");
    assert!(exists.is_none());
}
