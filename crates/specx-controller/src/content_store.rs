use sha2::{Digest, Sha256};
use specx_types::ContentRef;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub trait ContentStore {
    fn put(&self, bytes: &[u8], media_type: &str, created_at: &str) -> Result<ContentRef, ContentStoreError>;
    fn get(&self, content: &ContentRef) -> Result<Vec<u8>, ContentStoreError>;
    fn delete(&self, content: &ContentRef) -> Result<(), ContentStoreError>;
}

#[derive(Debug)]
pub enum ContentStoreError {
    Io(io::Error),
    InvalidReference,
    UnsupportedStore(String),
    HashMismatch,
}

impl std::fmt::Display for ContentStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "content store I/O failed: {error}"),
            Self::InvalidReference => formatter.write_str("invalid content reference"),
            Self::UnsupportedStore(kind) => write!(formatter, "unsupported content store: {kind}"),
            Self::HashMismatch => formatter.write_str("content hash does not match reference"),
        }
    }
}

impl std::error::Error for ContentStoreError {}

impl From<io::Error> for ContentStoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// ContentStore implementation for local and single-host deployments.
/// Objects are addressed by SHA-256 and written atomically, so repeated puts
/// are idempotent and interrupted writes do not become visible.
#[derive(Clone, Debug)]
pub struct FileContentStore {
    root: PathBuf,
}

impl FileContentStore {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, ContentStoreError> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self { root: root.as_ref().to_path_buf() })
    }

    fn object_path(&self, key: &str) -> Result<PathBuf, ContentStoreError> {
        let Some(hash) = key.strip_prefix("sha256/") else {
            return Err(ContentStoreError::InvalidReference);
        };
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ContentStoreError::InvalidReference);
        }
        Ok(self.root.join(hash))
    }
}

impl ContentStore for FileContentStore {
    fn put(&self, bytes: &[u8], media_type: &str, created_at: &str) -> Result<ContentRef, ContentStoreError> {
        let digest = Sha256::digest(bytes);
        let hash = format!("{digest:x}");
        let object_key = format!("sha256/{hash}");
        let destination = self.object_path(&object_key)?;
        if !destination.exists() {
            let temporary = self.root.join(format!(".{hash}.{}.tmp", std::process::id()));
            fs::write(&temporary, bytes)?;
            match fs::rename(&temporary, &destination) {
                Ok(()) => {}
                Err(error) if destination.exists() => {
                    let _ = fs::remove_file(&temporary);
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(ContentStoreError::Io(error));
                    }
                }
                Err(error) => return Err(ContentStoreError::Io(error)),
            }
        }
        Ok(ContentRef {
            store_kind: "filesystem".into(),
            object_key,
            media_type: media_type.into(),
            byte_length: bytes.len() as u64,
            content_hash: hash,
            created_at: created_at.into(),
            retention: None,
        })
    }

    fn get(&self, content: &ContentRef) -> Result<Vec<u8>, ContentStoreError> {
        if content.store_kind != "filesystem" {
            return Err(ContentStoreError::UnsupportedStore(content.store_kind.clone()));
        }
        let path = self.object_path(&content.object_key)?;
        let bytes = fs::read(path)?;
        let digest = Sha256::digest(&bytes);
        if format!("{digest:x}") != content.content_hash || bytes.len() as u64 != content.byte_length {
            return Err(ContentStoreError::HashMismatch);
        }
        Ok(bytes)
    }

    fn delete(&self, content: &ContentRef) -> Result<(), ContentStoreError> {
        if content.store_kind != "filesystem" {
            return Err(ContentStoreError::UnsupportedStore(content.store_kind.clone()));
        }
        let path = self.object_path(&content.object_key)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(ContentStoreError::Io(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_put_is_content_addressed_and_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let store = FileContentStore::new(directory.path()).unwrap();
        let first = store.put(b"hello", "text/plain", "2026-01-01T00:00:00Z").unwrap();
        let second = store.put(b"hello", "text/plain", "2026-01-01T00:00:01Z").unwrap();
        assert_eq!(first.object_key, second.object_key);
        assert_eq!(store.get(&first).unwrap(), b"hello");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn content_reference_rejects_tampering_and_path_traversal() {
        let directory = tempfile::tempdir().unwrap();
        let store = FileContentStore::new(directory.path()).unwrap();
        let mut reference = store.put(b"hello", "text/plain", "2026-01-01T00:00:00Z").unwrap();
        reference.content_hash = "0".repeat(64);
        assert!(matches!(store.get(&reference), Err(ContentStoreError::HashMismatch)));
        reference.object_key = "sha256/../secret".into();
        assert!(matches!(store.get(&reference), Err(ContentStoreError::InvalidReference)));
    }
}
