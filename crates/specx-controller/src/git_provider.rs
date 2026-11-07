use sha2::{Digest, Sha256};
use specx_formats::normalize_line_endings;
use specx_types::SpecificationRefKind;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitFileRequest {
    pub repository_url: String,
    pub ref_kind: SpecificationRefKind,
    pub ref_name: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitFileSnapshot {
    pub resolved_commit: String,
    pub manifest_hash: String,
    pub content: String,
    pub content_hash: String,
    pub provider_version: String,
}

pub trait GitProvider: Send + Sync {
    fn read_markdown_file(&self, request: &GitFileRequest) -> Result<GitFileSnapshot, GitProviderError>;
}

#[derive(Debug)]
pub enum GitProviderError {
    InvalidRequest,
    GitUnavailable,
    CommandFailed { operation: &'static str, detail: String },
    FileNotFound,
    DocumentTooLarge,
    InvalidUtf8,
    Io(io::Error),
}

impl GitProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_source_request",
            Self::GitUnavailable => "git_provider_unavailable",
            Self::CommandFailed { operation: "clone", .. } => "repository_fetch_failed",
            Self::CommandFailed { operation: "fetch", .. } => "repository_fetch_failed",
            Self::CommandFailed { operation: "resolve", .. } => "repository_ref_unavailable",
            Self::CommandFailed { operation: "read_file", .. } => "repository_file_unavailable",
            Self::CommandFailed { .. } => "git_provider_failed",
            Self::FileNotFound => "repository_file_not_found",
            Self::DocumentTooLarge => "repository_document_too_large",
            Self::InvalidUtf8 => "repository_document_not_utf8",
            Self::Io(_) => "git_provider_io_failed",
        }
    }
}

impl std::fmt::Display for GitProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest => formatter.write_str("the Git file request is invalid"),
            Self::GitUnavailable => formatter.write_str("the git executable is unavailable"),
            Self::CommandFailed { operation, detail } => write!(formatter, "git {operation} failed: {detail}"),
            Self::FileNotFound => formatter.write_str("the Markdown file was not found"),
            Self::DocumentTooLarge => formatter.write_str("the Markdown document exceeds the size limit"),
            Self::InvalidUtf8 => formatter.write_str("the Markdown document is not valid UTF-8"),
            Self::Io(error) => write!(formatter, "Git provider I/O failed: {error}"),
        }
    }
}

impl std::error::Error for GitProviderError {}

impl From<io::Error> for GitProviderError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Git CLI provider used by the first controller implementation. It keeps the
/// repository checkout private to the provider and returns only the requested
/// Markdown file plus immutable revision evidence.
#[derive(Clone, Debug)]
pub struct CommandGitProvider {
    git_binary: String,
    temp_root: PathBuf,
}

impl CommandGitProvider {
    pub fn new(temp_root: impl AsRef<Path>) -> Result<Self, GitProviderError> {
        fs::create_dir_all(temp_root.as_ref())?;
        let git_binary = std::env::var("SPECX_GIT_BIN").unwrap_or_else(|_| "git".into());
        if Command::new(&git_binary).arg("--version").output().is_err() {
            return Err(GitProviderError::GitUnavailable);
        }
        Ok(Self { git_binary, temp_root: temp_root.as_ref().to_path_buf() })
    }

    fn checkout_path(&self, request: &GitFileRequest) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |duration| duration.as_nanos());
        let digest =
            Sha256::digest(format!("{}\0{}\0{}\0{stamp}", request.repository_url, request.ref_name, request.path));
        self.temp_root.join(format!("sync-{digest:x}"))
    }

    fn run_git(
        &self,
        directory: Option<&Path>,
        args: &[&str],
        operation: &'static str,
    ) -> Result<Vec<u8>, GitProviderError> {
        let mut command = Command::new(&self.git_binary);
        if let Some(directory) = directory {
            command.arg("-C").arg(directory);
        }
        let output = command.args(args).output().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GitProviderError::GitUnavailable
            } else {
                GitProviderError::Io(error)
            }
        })?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().chars().take(500).collect();
            return Err(GitProviderError::CommandFailed { operation, detail });
        }
        Ok(output.stdout)
    }

    fn resolve_and_read(
        &self,
        request: &GitFileRequest,
        checkout: &Path,
    ) -> Result<(String, String, Vec<u8>), GitProviderError> {
        match request.ref_kind {
            SpecificationRefKind::Branch | SpecificationRefKind::Tag => {
                let ref_name = request.ref_name.as_str();
                self.run_git(
                    None,
                    &[
                        "clone",
                        "--quiet",
                        "--depth",
                        "1",
                        "--no-checkout",
                        "--branch",
                        ref_name,
                        &request.repository_url,
                        checkout.to_str().ok_or(GitProviderError::InvalidRequest)?,
                    ],
                    "clone",
                )?;
                let commit = self.run_git(Some(checkout), &["rev-parse", "HEAD"], "resolve")?;
                let commit = String::from_utf8(commit).map_err(|_| GitProviderError::InvalidUtf8)?.trim().to_owned();
                let manifest = self.run_git(Some(checkout), &["ls-tree", "-r", "--full-tree", "HEAD"], "resolve")?;
                let pathspec = format!("HEAD:{}", request.path);
                let bytes =
                    self.run_git(Some(checkout), &["show", "--format=", "--no-ext-diff", &pathspec], "read_file")?;
                Ok((commit, manifest_hash(&manifest), bytes))
            }
            SpecificationRefKind::Commit => {
                self.run_git(
                    None,
                    &["init", "--quiet", checkout.to_str().ok_or(GitProviderError::InvalidRequest)?],
                    "clone",
                )?;
                self.run_git(Some(checkout), &["remote", "add", "origin", &request.repository_url], "clone")?;
                self.run_git(
                    Some(checkout),
                    &["fetch", "--quiet", "--depth", "1", "origin", &request.ref_name],
                    "fetch",
                )?;
                let commit = self.run_git(Some(checkout), &["rev-parse", "FETCH_HEAD^{commit}"], "resolve")?;
                let commit = String::from_utf8(commit).map_err(|_| GitProviderError::InvalidUtf8)?.trim().to_owned();
                let manifest =
                    self.run_git(Some(checkout), &["ls-tree", "-r", "--full-tree", "FETCH_HEAD"], "resolve")?;
                let pathspec = format!("FETCH_HEAD:{}", request.path);
                let bytes =
                    self.run_git(Some(checkout), &["show", "--format=", "--no-ext-diff", &pathspec], "read_file")?;
                Ok((commit, manifest_hash(&manifest), bytes))
            }
        }
    }
}

impl GitProvider for CommandGitProvider {
    fn read_markdown_file(&self, request: &GitFileRequest) -> Result<GitFileSnapshot, GitProviderError> {
        validate_markdown_path(&request.path)?;
        if request.repository_url.trim().is_empty() || request.ref_name.trim().is_empty() {
            return Err(GitProviderError::InvalidRequest);
        }
        let checkout = self.checkout_path(request);
        let result = self.resolve_and_read(request, &checkout).and_then(|(resolved_commit, manifest_hash, bytes)| {
            if bytes.len() > MAX_DOCUMENT_BYTES {
                return Err(GitProviderError::DocumentTooLarge);
            }
            let content = String::from_utf8(bytes).map_err(|_| GitProviderError::InvalidUtf8)?;
            let content = normalize_line_endings(&content);
            if content.trim().is_empty() {
                return Err(GitProviderError::InvalidRequest);
            }
            let digest = Sha256::digest(content.as_bytes());
            Ok(GitFileSnapshot {
                resolved_commit,
                manifest_hash,
                content_hash: format!("sha256:{digest:x}"),
                content,
                provider_version: "git-cli".into(),
            })
        });
        let _ = fs::remove_dir_all(&checkout);
        result
    }
}

fn manifest_hash(manifest: &[u8]) -> String {
    let digest = Sha256::digest(manifest);
    format!("sha256:{digest:x}")
}

pub fn validate_markdown_path(path: &str) -> Result<(), GitProviderError> {
    let lower = path.to_ascii_lowercase();
    if path.trim().is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains("..")
        || path.chars().any(char::is_control)
        || path.chars().any(|character| matches!(character, '~' | '^' | ':' | '?' | '*' | '['))
        || path.split('/').any(|part| part.is_empty() || part == ".")
        || !(lower.ends_with(".md") || lower.ends_with(".markdown"))
    {
        return Err(GitProviderError::InvalidRequest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_relative_markdown_paths() {
        assert!(validate_markdown_path("docs/spec.md").is_ok());
        assert!(validate_markdown_path("docs/spec.markdown").is_ok());
        assert!(validate_markdown_path("../spec.md").is_err());
        assert!(validate_markdown_path("/spec.md").is_err());
        assert!(validate_markdown_path("docs/spec*.md").is_err());
        assert!(validate_markdown_path("docs/spec:latest.md").is_err());
        assert!(validate_markdown_path("docs/spec.txt").is_err());
    }

    #[test]
    fn reads_a_file_from_a_real_local_git_repository() {
        let repository = tempfile::tempdir().unwrap();
        let checkout = repository.path().join("source");
        std::fs::create_dir_all(checkout.join("docs")).unwrap();
        std::fs::write(checkout.join("docs/spec.md"), "# Hello\r\n").unwrap();
        run(&checkout, &["init", "--quiet"]);
        run(&checkout, &["config", "user.email", "test@example.com"]);
        run(&checkout, &["config", "user.name", "SpecX Test"]);
        run(&checkout, &["add", "."]);
        run(&checkout, &["commit", "--quiet", "-m", "initial"]);
        let provider = CommandGitProvider::new(repository.path().join("work")).unwrap();
        let snapshot = provider
            .read_markdown_file(&GitFileRequest {
                repository_url: checkout.to_string_lossy().into_owned(),
                ref_kind: SpecificationRefKind::Branch,
                ref_name: "master".into(),
                path: "docs/spec.md".into(),
            })
            .or_else(|error| {
                if error.code() == "repository_fetch_failed" {
                    provider.read_markdown_file(&GitFileRequest {
                        ref_name: "main".into(),
                        ..GitFileRequest {
                            repository_url: checkout.to_string_lossy().into_owned(),
                            ref_kind: SpecificationRefKind::Branch,
                            ref_name: "main".into(),
                            path: "docs/spec.md".into(),
                        }
                    })
                } else {
                    Err(error)
                }
            })
            .unwrap();
        assert_eq!(snapshot.content, "# Hello\n");
        assert_eq!(snapshot.resolved_commit.len(), 40);
    }

    fn run(directory: &Path, args: &[&str]) {
        let status = Command::new("git").current_dir(directory).args(args).status().unwrap();
        assert!(status.success());
    }
}
