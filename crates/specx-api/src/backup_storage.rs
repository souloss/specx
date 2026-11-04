use specx_core::persistence::PersistenceError;

#[derive(Clone)]
pub(super) struct BackupObject {
    pub file_name: String,
    pub bytes: u64,
}

#[cfg(feature = "s3-backup")]
#[derive(Clone)]
pub(super) struct BackupStorage {
    client: aws_sdk_s3::Client,
    bucket: String,
    namespace: String,
    server_side_encryption: Option<aws_sdk_s3::types::ServerSideEncryption>,
    kms_key_id: Option<String>,
    retention_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
}

#[cfg(feature = "s3-backup")]
impl BackupStorage {
    pub(super) fn from_env() -> Result<Option<Self>, PersistenceError> {
        use aws_sdk_s3::config::{Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation};

        let names = [
            "SPECX_BACKUP_S3_ENDPOINT",
            "SPECX_BACKUP_S3_BUCKET",
            "SPECX_BACKUP_S3_ACCESS_KEY_ID",
            "SPECX_BACKUP_S3_SECRET_ACCESS_KEY",
            "SPECX_BACKUP_S3_SESSION_TOKEN",
            "SPECX_BACKUP_S3_REGION",
            "SPECX_BACKUP_S3_SSE",
            "SPECX_BACKUP_S3_KMS_KEY_ID",
        ];
        if !names.iter().any(|name| std::env::var_os(name).is_some()) {
            return Ok(None);
        }

        let required = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| PersistenceError::new(format!("{name} is required when S3 backup storage is enabled")))
        };
        let endpoint = required("SPECX_BACKUP_S3_ENDPOINT")?;
        validate_endpoint(&endpoint)?;
        let bucket = required("SPECX_BACKUP_S3_BUCKET")?;
        validate_bucket(&bucket)?;
        let access_key = required("SPECX_BACKUP_S3_ACCESS_KEY_ID")?;
        let secret_key = required("SPECX_BACKUP_S3_SECRET_ACCESS_KEY")?;
        let region = std::env::var("SPECX_BACKUP_S3_REGION")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "us-east-1".to_owned());
        let session_token = std::env::var("SPECX_BACKUP_S3_SESSION_TOKEN")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let (server_side_encryption, kms_key_id) = match std::env::var("SPECX_BACKUP_S3_SSE")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .as_deref()
        {
            None => (None, None),
            Some("aes256") => (Some(aws_sdk_s3::types::ServerSideEncryption::Aes256), None),
            Some("aws:kms") => {
                let key = std::env::var("SPECX_BACKUP_S3_KMS_KEY_ID")
                    .ok()
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty());
                (Some(aws_sdk_s3::types::ServerSideEncryption::AwsKms), key)
            }
            Some(_) => {
                return Err(PersistenceError::new("SPECX_BACKUP_S3_SSE must be AES256 or aws:kms"));
            }
        };
        if std::env::var_os("SPECX_BACKUP_S3_KMS_KEY_ID").is_some() && kms_key_id.is_none() {
            return Err(PersistenceError::new(
                "SPECX_BACKUP_S3_KMS_KEY_ID can only be set with SPECX_BACKUP_S3_SSE=aws:kms",
            ));
        }
        let credentials = Credentials::new(access_key, secret_key, session_token, None, "specx-backup");
        let config = aws_sdk_s3::Config::builder()
            .behavior_version_latest()
            .region(Region::new(region))
            .credentials_provider(credentials)
            .endpoint_url(endpoint)
            .force_path_style(true)
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .build();
        Ok(Some(Self {
            client: aws_sdk_s3::Client::from_conf(config),
            bucket,
            namespace: "specx".to_owned(),
            server_side_encryption,
            kms_key_id,
            retention_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        }))
    }

    pub(super) async fn validate(&self) -> Result<(), PersistenceError> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| PersistenceError::new(format!("S3 backup bucket is unavailable: {error}")))?;
        Ok(())
    }

    pub(super) async fn put_backup(
        &self,
        scope_prefix: &str,
        file_name: &str,
        source: &std::path::Path,
        sha256: &str,
        retention: usize,
    ) -> Result<(), PersistenceError> {
        use aws_sdk_s3::primitives::ByteStream;

        if retention == 0 {
            return Err(PersistenceError::new("S3 backup retention must be at least one"));
        }
        let checksum = sha256.strip_prefix("sha256:").unwrap_or(sha256);
        if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(PersistenceError::new("specX backup SHA-256 value is malformed"));
        }
        let key = self.object_key(scope_prefix, file_name)?;
        let _guard = self.retention_lock.lock().await;
        let body = ByteStream::from_path(source)
            .await
            .map_err(|error| PersistenceError::new(format!("cannot read specX backup for S3 upload: {error}")))?;
        let mut request = self.client.put_object().bucket(&self.bucket).key(&key).body(body);
        if let Some(encryption) = self.server_side_encryption.clone() {
            request = request.server_side_encryption(encryption);
        }
        if let Some(key_id) = &self.kms_key_id {
            request = request.ssekms_key_id(key_id);
        }
        request.send().await.map_err(|error| PersistenceError::new(format!("S3 backup upload failed: {error}")))?;

        let mut checksum_upload = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(format!("{key}.sha256"))
            .body(ByteStream::from(checksum.as_bytes().to_vec()));
        if let Some(encryption) = self.server_side_encryption.clone() {
            checksum_upload = checksum_upload.server_side_encryption(encryption);
        }
        if let Some(key_id) = &self.kms_key_id {
            checksum_upload = checksum_upload.ssekms_key_id(key_id);
        }
        if let Err(error) = checksum_upload.send().await {
            let _ = self.client.delete_object().bucket(&self.bucket).key(&key).send().await;
            return Err(PersistenceError::new(format!("S3 backup checksum upload failed: {error}")));
        }

        let extension = if file_name.ends_with(".dump") { ".dump" } else { ".sqlite" };
        self.rotate_retention(scope_prefix, extension, retention).await
    }

    pub(super) async fn list_backups(
        &self,
        scope_prefix: &str,
        extension: &str,
    ) -> Result<Vec<BackupObject>, PersistenceError> {
        let _guard = self.retention_lock.lock().await;
        let mut objects = self.list_scope(scope_prefix, extension).await?;
        objects.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        Ok(objects)
    }

    pub(super) async fn restore_backup(
        &self,
        scope_prefix: &str,
        file_name: &str,
        destination: &std::path::Path,
    ) -> Result<(), PersistenceError> {
        use sha2::{Digest, Sha256};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _guard = self.retention_lock.lock().await;
        let key = self.object_key(scope_prefix, file_name)?;
        let checksum = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(format!("{key}.sha256"))
            .send()
            .await
            .map_err(|error| PersistenceError::new(format!("S3 backup checksum is unavailable: {error}")))?;
        let mut checksum_body = checksum.body.into_async_read();
        let mut checksum_bytes = Vec::with_capacity(64);
        checksum_body
            .read_to_end(&mut checksum_bytes)
            .await
            .map_err(|error| PersistenceError::new(format!("cannot read S3 backup checksum: {error}")))?;
        let expected_hash = String::from_utf8(checksum_bytes)
            .map_err(|_| PersistenceError::new("S3 backup checksum is not valid UTF-8"))?
            .trim()
            .to_owned();
        let expected_hash = expected_hash.strip_prefix("sha256:").unwrap_or(&expected_hash);
        if expected_hash.len() != 64 || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(PersistenceError::new(format!(
                "S3 backup checksum is malformed ({} bytes: {:?})",
                expected_hash.len(),
                expected_hash.chars().take(96).collect::<String>()
            )));
        }

        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| PersistenceError::new(format!("S3 backup download failed: {error}")))?;
        let mut body = output.body.into_async_read();
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .await
            .map_err(PersistenceError::from)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        let result = async {
            loop {
                let read = body.read(&mut buffer).await.map_err(PersistenceError::from)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                file.write_all(&buffer[..read]).await.map_err(PersistenceError::from)?;
            }
            file.flush().await.map_err(PersistenceError::from)?;
            let actual_hash = format!("{:x}", hasher.finalize());
            if actual_hash != expected_hash {
                return Err(PersistenceError::new("S3 backup SHA-256 metadata does not match object content"));
            }
            Ok(())
        }
        .await;
        drop(file);
        if result.is_err() {
            let _ = tokio::fs::remove_file(destination).await;
        }
        result
    }

    async fn list_scope(&self, scope_prefix: &str, extension: &str) -> Result<Vec<BackupObject>, PersistenceError> {
        let prefix = self.object_prefix(scope_prefix)?;
        let mut continuation_token = None;
        let mut objects = Vec::new();
        loop {
            let mut request = self.client.list_objects_v2().bucket(&self.bucket).prefix(&prefix);
            if let Some(token) = continuation_token.as_deref() {
                request = request.continuation_token(token);
            }
            let output = request
                .send()
                .await
                .map_err(|error| PersistenceError::new(format!("S3 backup listing failed: {error}")))?;
            for object in output.contents() {
                let Some(key) = object.key() else { continue };
                let Some(file_name) = key.strip_prefix(&prefix) else { continue };
                if !valid_file_name(file_name, extension) {
                    continue;
                }
                let Some(bytes) = object.size().and_then(|size| u64::try_from(size).ok()) else { continue };
                objects.push(BackupObject { file_name: file_name.to_owned(), bytes });
            }
            continuation_token = output.next_continuation_token().map(ToOwned::to_owned);
            if continuation_token.is_none() {
                break;
            }
        }
        Ok(objects)
    }

    async fn rotate_retention(
        &self,
        scope_prefix: &str,
        extension: &str,
        retention: usize,
    ) -> Result<(), PersistenceError> {
        let mut objects = self.list_scope(scope_prefix, extension).await?;
        objects.sort_by(|left, right| left.file_name.cmp(&right.file_name));
        let excess = objects.len().saturating_sub(retention);
        for object in objects.into_iter().take(excess) {
            let old_key = self.object_key(scope_prefix, &object.file_name)?;
            self.client
                .delete_object()
                .bucket(&self.bucket)
                .key(&old_key)
                .send()
                .await
                .map_err(|error| PersistenceError::new(format!("S3 backup retention cleanup failed: {error}")))?;
            self.client.delete_object().bucket(&self.bucket).key(format!("{old_key}.sha256")).send().await.map_err(
                |error| PersistenceError::new(format!("S3 backup checksum retention cleanup failed: {error}")),
            )?;
        }
        Ok(())
    }

    fn object_prefix(&self, scope_prefix: &str) -> Result<String, PersistenceError> {
        if !valid_scope_prefix(scope_prefix) {
            return Err(PersistenceError::new("invalid specX backup scope prefix"));
        }
        Ok(format!("{}/{}/", self.namespace, scope_prefix))
    }

    fn object_key(&self, scope_prefix: &str, file_name: &str) -> Result<String, PersistenceError> {
        if !valid_file_name(file_name, ".sqlite") && !valid_file_name(file_name, ".dump") {
            return Err(PersistenceError::new("invalid specX backup object name"));
        }
        Ok(format!("{}{file_name}", self.object_prefix(scope_prefix)?))
    }
}

#[cfg(not(feature = "s3-backup"))]
#[derive(Clone)]
pub(super) struct BackupStorage;

#[cfg(not(feature = "s3-backup"))]
impl BackupStorage {
    pub(super) fn from_env() -> Result<Option<Self>, PersistenceError> {
        if [
            "SPECX_BACKUP_S3_ENDPOINT",
            "SPECX_BACKUP_S3_BUCKET",
            "SPECX_BACKUP_S3_ACCESS_KEY_ID",
            "SPECX_BACKUP_S3_SECRET_ACCESS_KEY",
            "SPECX_BACKUP_S3_SESSION_TOKEN",
            "SPECX_BACKUP_S3_REGION",
            "SPECX_BACKUP_S3_SSE",
            "SPECX_BACKUP_S3_KMS_KEY_ID",
        ]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
        {
            return Err(PersistenceError::new(
                "S3 backup environment is set, but specx-web was built without the s3-backup feature",
            ));
        }
        Ok(None)
    }

    pub(super) async fn validate(&self) -> Result<(), PersistenceError> {
        Ok(())
    }
}

#[cfg(feature = "s3-backup")]
fn valid_scope_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 80
        && prefix.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(feature = "s3-backup")]
fn valid_file_name(file_name: &str, extension: &str) -> bool {
    !file_name.is_empty()
        && file_name.len() <= 160
        && file_name.ends_with(extension)
        && file_name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(feature = "s3-backup")]
fn validate_endpoint(endpoint: &str) -> Result<(), PersistenceError> {
    let uri = endpoint
        .parse::<http::Uri>()
        .map_err(|_| PersistenceError::new("SPECX_BACKUP_S3_ENDPOINT must be an absolute HTTP(S) URL"))?;
    let valid_scheme = uri.scheme_str().is_some_and(|scheme| matches!(scheme, "http" | "https"));
    let valid_authority = uri.authority().is_some_and(|authority| !authority.as_str().contains('@'));
    let valid_path = uri.path_and_query().is_none_or(|path| path.as_str() == "/");
    let local_http = uri.scheme_str() == Some("http")
        && uri.host().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host.parse::<std::net::IpAddr>().is_ok_and(|address| address.is_loopback())
        });
    let encrypted_transport = uri.scheme_str() == Some("https") || local_http;
    if !valid_scheme || !valid_authority || !valid_path || !encrypted_transport {
        return Err(PersistenceError::new(
            "SPECX_BACKUP_S3_ENDPOINT must be an HTTPS origin (or loopback HTTP for local testing) without credentials, path, or query",
        ));
    }
    Ok(())
}

#[cfg(feature = "s3-backup")]
fn validate_bucket(bucket: &str) -> Result<(), PersistenceError> {
    let starts_and_ends_with_alphanumeric = bucket
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        && bucket.chars().last().is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit());
    let valid = (3..=63).contains(&bucket.len())
        && starts_and_ends_with_alphanumeric
        && bucket.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
        && !bucket.contains("..")
        && !bucket.contains(".-")
        && !bucket.contains("-.");
    if !valid {
        return Err(PersistenceError::new("SPECX_BACKUP_S3_BUCKET is not a valid S3 bucket name"));
    }
    Ok(())
}

#[cfg(all(test, feature = "s3-backup"))]
mod tests {
    use super::*;
    use sha2::Digest;
    use std::io::Write;

    #[test]
    fn s3_endpoint_and_bucket_configuration_is_constrained() {
        assert!(validate_endpoint("http://127.0.0.1:19000").is_ok());
        assert!(validate_endpoint("http://objects.example.com").is_err());
        assert!(validate_endpoint("https://objects.example.com/").is_ok());
        assert!(validate_endpoint("https://user:secret@objects.example.com").is_err());
        assert!(validate_endpoint("https://objects.example.com/prefix").is_err());
        assert!(validate_endpoint("https://objects.example.com?tenant=1").is_err());
        assert!(validate_endpoint("ftp://objects.example.com").is_err());
        assert!(validate_bucket("specx-backups-prod").is_ok());
        assert!(validate_bucket("UPPERCASE").is_err());
        assert!(validate_bucket("bad..bucket").is_err());
    }

    #[tokio::test]
    async fn configured_s3_storage_round_trips_and_rotates_objects() {
        use axum::{body::Body, http::Request};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        if std::env::var_os("SPECX_BACKUP_S3_ENDPOINT").is_none() {
            eprintln!("S3 backup integration skipped: SPECX_BACKUP_S3_ENDPOINT is not configured");
            return;
        }
        let storage = BackupStorage::from_env().unwrap().unwrap();
        if std::env::var_os("SPECX_TEST_S3_DELETE_BUCKET").is_some() {
            storage.validate().await.unwrap();
            let mut continuation_token = None;
            loop {
                let mut request = storage.client.list_objects_v2().bucket(&storage.bucket).prefix("specx/");
                if let Some(token) = continuation_token.as_deref() {
                    request = request.continuation_token(token);
                }
                let output = request.send().await.unwrap();
                for object in output.contents() {
                    if let Some(key) = object.key() {
                        storage.client.delete_object().bucket(&storage.bucket).key(key).send().await.unwrap();
                    }
                }
                continuation_token = output.next_continuation_token().map(ToOwned::to_owned);
                if continuation_token.is_none() {
                    break;
                }
            }
            storage.client.delete_bucket().bucket(&storage.bucket).send().await.unwrap();
            return;
        }
        let created_bucket = storage.validate().await.is_err();
        if created_bucket {
            storage.client.create_bucket().bucket(&storage.bucket).send().await.unwrap();
            storage.validate().await.unwrap();
        }

        let scope = format!("specx-test-{}", uuid::Uuid::new_v4().simple());
        let prefix = storage.object_prefix(&scope).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let mut files = Vec::new();
        for index in 0..3 {
            let file_name = format!("{scope}-{index}.sqlite");
            let bytes = format!("specX backup object {index}").into_bytes();
            let mut file = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
            file.write_all(&bytes).unwrap();
            let sha256 = format!("{:x}", sha2::Sha256::digest(&bytes));
            storage.put_backup(&scope, &file_name, file.path(), &sha256, 2).await.unwrap();
            files.push((file_name, bytes));
        }

        let listed = storage.list_backups(&scope, ".sqlite").await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].file_name, files[1].0);
        assert_eq!(listed[1].file_name, files[2].0);
        let destination = directory.path().join("restored.sqlite");
        storage.restore_backup(&scope, &files[2].0, &destination).await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), files[2].1);

        let concurrent_scope = format!("{scope}-concurrent");
        let mut concurrent = tokio::task::JoinSet::new();
        for index in 0..8 {
            let file_name = format!("{concurrent_scope}-{index:02}.sqlite");
            let bytes = format!("Concurrent specX backup object {index}").into_bytes();
            let source = directory.path().join(format!("source-{index:02}.sqlite"));
            std::fs::write(&source, &bytes).unwrap();
            let sha256 = format!("{:x}", sha2::Sha256::digest(&bytes));
            let storage = storage.clone();
            let concurrent_scope = concurrent_scope.clone();
            concurrent
                .spawn(async move { storage.put_backup(&concurrent_scope, &file_name, &source, &sha256, 2).await });
        }
        while let Some(result) = concurrent.join_next().await {
            result.unwrap().unwrap();
        }
        let concurrent_objects = storage.list_backups(&concurrent_scope, ".sqlite").await.unwrap();
        assert_eq!(concurrent_objects.len(), 2);
        assert_eq!(concurrent_objects[0].file_name, format!("{concurrent_scope}-06.sqlite"));
        assert_eq!(concurrent_objects[1].file_name, format!("{concurrent_scope}-07.sqlite"));

        for object in listed {
            let key = storage.object_key(&scope, &object.file_name).unwrap();
            storage.client.delete_object().bucket(&storage.bucket).key(&key).send().await.unwrap();
            storage.client.delete_object().bucket(&storage.bucket).key(format!("{key}.sha256")).send().await.unwrap();
        }

        let tenant = format!("s3-integration-{}", uuid::Uuid::new_v4().simple());
        let backup_root = directory.path().join("api-backups");
        let state = crate::ApiState::open_with_backup_root(directory.path().join("api.sqlite"), &backup_root)
            .unwrap()
            .with_backup_storage_from_env()
            .await
            .unwrap()
            .with_backup_retention(2)
            .unwrap();
        let app = crate::router::<()>(state.clone());
        let request = |method, path: String| {
            Request::builder()
                .method(method)
                .uri(path)
                .header("x-specx-tenant-id", &tenant)
                .header("x-specx-workspace-id", "default")
                .header("x-specx-actor-id", "owner")
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(
            app.clone().oneshot(request("GET", "/catalog".to_owned())).await.unwrap().status(),
            axum::http::StatusCode::OK
        );
        let mut api_file_names = Vec::new();
        for _ in 0..3 {
            let created = app.clone().oneshot(request("POST", "/backups".to_owned())).await.unwrap();
            assert_eq!(created.status(), axum::http::StatusCode::OK);
            let created: serde_json::Value =
                serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
            api_file_names.push(created["fileName"].as_str().unwrap().to_owned());
        }
        let api_scope = crate::backup_prefix(&crate::WorkspaceContext {
            tenant_id: tenant.clone(),
            workspace_id: "default".to_owned(),
            actor_id: "owner".to_owned(),
        });
        let listed = app.clone().oneshot(request("GET", "/backups".to_owned())).await.unwrap();
        assert_eq!(listed.status(), axum::http::StatusCode::OK);
        let listed: serde_json::Value =
            serde_json::from_slice(&listed.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(listed.as_array().unwrap().len(), 2);
        assert!(listed.as_array().unwrap().iter().any(|entry| entry["fileName"] == api_file_names[2]));
        assert!(!listed.as_array().unwrap().iter().any(|entry| entry["fileName"] == api_file_names[0]));
        let downloaded = directory.path().join("api-backup-downloaded.sqlite");
        storage.restore_backup(&api_scope, &api_file_names[2], &downloaded).await.unwrap();
        let source = backup_root.join(&api_file_names[2]);
        assert_eq!(std::fs::read(&source).unwrap(), std::fs::read(&downloaded).unwrap());
        {
            let context = crate::WorkspaceContext {
                tenant_id: tenant.clone(),
                workspace_id: "default".to_owned(),
                actor_id: "owner".to_owned(),
            };
            let mut inner = state.inner.lock().await;
            inner.store.restore_workspace_from_path(&context, &downloaded).unwrap();
        }
        let restored =
            app.clone().oneshot(request("POST", format!("/backups/{}/restore", api_file_names[2]))).await.unwrap();
        let restored_status = restored.status();
        if restored_status != axum::http::StatusCode::OK {
            let message =
                String::from_utf8_lossy(&restored.into_body().collect().await.unwrap().to_bytes()).to_string();
            panic!("S3-backed API restore failed ({restored_status}): {message}");
        }

        for scope in [&scope, &api_scope, &concurrent_scope] {
            for object in storage.list_backups(scope, ".sqlite").await.unwrap() {
                let key = storage.object_key(scope, &object.file_name).unwrap();
                storage.client.delete_object().bucket(&storage.bucket).key(&key).send().await.unwrap();
                storage
                    .client
                    .delete_object()
                    .bucket(&storage.bucket)
                    .key(format!("{key}.sha256"))
                    .send()
                    .await
                    .unwrap();
            }
            storage
                .client
                .delete_object()
                .bucket(&storage.bucket)
                .key(format!("{}retention.lock", storage.object_prefix(scope).unwrap()))
                .send()
                .await
                .unwrap();
        }
        if created_bucket && std::env::var_os("SPECX_TEST_S3_PRESERVE_BUCKET").is_none() {
            storage.client.delete_bucket().bucket(&storage.bucket).send().await.unwrap();
        }
        assert!(prefix.starts_with("specx/"));
    }
}
