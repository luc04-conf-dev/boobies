use std::{
    cmp::Ordering,
    fs::{self, File},
    io::{BufReader, Read, copy},
    path::{Component, Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use url::Url;

use crate::{
    database,
    models::{RepositoryIndex, RepositoryPackage},
    package::sha256_file,
};

// ============================================================================
// Constants
// ============================================================================

const BOOBIES_VERSION: &str = "0.1.0";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 60);

const MAX_REDIRECTS: usize = 10;

const INDEX_FILE_NAME: &str = "database.json";
const LOCAL_PACKAGES_DIR: &str = "packages";

// ============================================================================
// Repository URL / path handling
// ============================================================================

/// Returns true when the repository string is an HTTP(S) URL.
///
/// This intentionally accepts both http:// and https://.
///
/// The check is case-insensitive so values such as:
///
///     HTTPS://example.com/repo
///
/// are accepted too.
fn is_http_repository(repository: &str) -> bool {
    let trimmed = repository.trim();

    let lower = trimmed.to_ascii_lowercase();

    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Normalize a repository base URL/path.
///
/// Examples:
///
///     https://example.com
///     https://example.com/
///
/// both become:
///
///     https://example.com/
///
/// The same behavior applies to local filesystem paths.
///
/// Empty or whitespace-only repositories are rejected.
pub fn normalize_base(base: &str) -> Result<String> {
    let trimmed = base.trim();

    if trimmed.is_empty() {
        bail!("repository URL/path cannot be empty");
    }

    let normalized = trimmed.trim_end_matches('/');

    if normalized.is_empty() {
        bail!("repository URL/path cannot be empty");
    }

    Ok(format!("{normalized}/"))
}

/// Parse and validate an HTTP(S) repository URL.
///
/// This gives us a proper `Url` instead of manually concatenating arbitrary
/// strings whenever possible.
fn parse_repository_url(repository: &str) -> Result<Url> {
    let normalized = normalize_base(repository)?;

    let url =
        Url::parse(&normalized).with_context(|| format!("invalid repository URL: {repository}"))?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => bail!("unsupported repository URL scheme: {scheme}"),
    }

    if url.host_str().is_none() {
        bail!("repository URL has no host: {repository}");
    }

    Ok(url)
}

/// Build the URL of the repository index.
fn repository_index_url(repository: &str) -> Result<Url> {
    let base = parse_repository_url(repository)?;

    base.join(INDEX_FILE_NAME)
        .with_context(|| format!("failed to build repository index URL from {base}"))
}

/// Build a package URL from a repository base and package URL.
///
/// This helper is currently mostly defensive because package entries are
/// expected to contain their own download URL. It is kept here to make URL
/// handling centralized and reusable.
fn resolve_package_url(base: &str, package_url: &str) -> Result<Url> {
    let parsed =
        Url::parse(package_url).with_context(|| format!("invalid package URL: {package_url}"))?;

    if parsed.scheme() == "http" || parsed.scheme() == "https" {
        return Ok(parsed);
    }

    let base_url = parse_repository_url(base)?;

    base_url
        .join(package_url)
        .with_context(|| format!("failed to resolve package URL: {package_url}"))
}

// ============================================================================
// HTTP client
// ============================================================================

/// Build the HTTP client used by Boobies.
///
/// The client is intentionally configured in one place so repository index
/// fetching and package downloads behave consistently.
fn build_http_client(timeout: Duration) -> Result<Client> {
    let mut headers = HeaderMap::new();

    headers.insert(USER_AGENT, HeaderValue::from_static("boobies/0.1.0"));

    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "application/json, application/octet-stream, application/x-tar, */*",
        ),
    );

    Client::builder()
        .default_headers(headers)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .build()
        .context("failed to build HTTP client")
}

/// Ensure a response is successful and provide useful context.
fn ensure_success(response: Response, url: &Url) -> Result<Response> {
    let status = response.status();

    if !status.is_success() {
        bail!("repository request failed for {url}: HTTP {}", status);
    }

    Ok(response)
}

// ============================================================================
// Repository index fetching
// ============================================================================

/// Fetch the repository index from either:
///
///     http(s)://.../database.json
///
/// or:
///
///     /local/path/database.json
///
/// The resulting JSON is deserialized directly into RepositoryIndex.
pub fn fetch_repository_index(repository: &str) -> Result<RepositoryIndex> {
    let repository = repository.trim();

    if repository.is_empty() {
        bail!("repository URL/path cannot be empty");
    }

    if is_http_repository(repository) {
        fetch_remote_repository_index(repository)
    } else {
        fetch_local_repository_index(repository)
    }
}

/// Fetch repository index over HTTP(S).
fn fetch_remote_repository_index(repository: &str) -> Result<RepositoryIndex> {
    let url = repository_index_url(repository)?;

    let client = build_http_client(REQUEST_TIMEOUT)?;

    let response = client
        .get(url.clone())
        .send()
        .with_context(|| format!("failed to GET repository index {url}"))?;

    let response = ensure_success(response, &url)?;

    let index: RepositoryIndex = response
        .json()
        .with_context(|| format!("failed to parse repository JSON from {url}"))?;

    validate_repository_index(&index)
        .with_context(|| format!("repository index validation failed for {url}"))?;

    Ok(index)
}

/// Load repository index from a local filesystem repository.
fn fetch_local_repository_index(repository: &str) -> Result<RepositoryIndex> {
    let base = PathBuf::from(repository);

    if !base.exists() {
        bail!("local repository does not exist: {}", base.display());
    }

    if !base.is_dir() {
        bail!("local repository is not a directory: {}", base.display());
    }

    let path = base.join(INDEX_FILE_NAME);

    if !path.exists() {
        bail!("local repository database not found: {}", path.display());
    }

    if !path.is_file() {
        bail!(
            "local repository database is not a regular file: {}",
            path.display()
        );
    }

    let index: RepositoryIndex =
        database::load_json(&path).with_context(|| format!("failed to load {}", path.display()))?;

    validate_repository_index(&index)
        .with_context(|| format!("repository index validation failed for {}", path.display()))?;

    Ok(index)
}

/// Validate the basic invariants of a repository index.
///
/// This intentionally does not require package names to match a particular
/// character set. Hyphens, underscores, periods, plus signs, and other
/// characters are allowed.
fn validate_repository_index(index: &RepositoryIndex) -> Result<()> {
    for (position, package) in index.packages.iter().enumerate() {
        if package.name.trim().is_empty() {
            bail!("package at index {position} has an empty name");
        }

        if package.version.trim().is_empty() {
            bail!("package `{}` has an empty version", package.name);
        }

        if package.architecture.trim().is_empty() {
            bail!("package `{}` has an empty architecture", package.name);
        }

        if package.filename.trim().is_empty() {
            bail!("package `{}` has an empty filename", package.name);
        }

        if is_http_download_url(&package.download_url)
            && Url::parse(package.download_url.trim()).is_err()
        {
            bail!(
                "package `{}` has an invalid download URL: {}",
                package.name,
                package.download_url
            );
        }

        if !package.sha256.trim().is_empty() && !is_valid_sha256(package.sha256.trim()) {
            bail!("package `{}` has an invalid SHA-256 value", package.name);
        }
    }

    Ok(())
}

// ============================================================================
// Downloading packages
// ============================================================================

/// Download or copy a package to `destination`.
///
/// For HTTP(S) repositories, package.download_url is used directly.
///
/// For local repositories, the package is loaded from:
///
///     <repository>/packages/<filename>
///
/// Downloads are written to a temporary file and atomically renamed when
/// possible. This prevents a failed/interrupted download from leaving a
/// partially valid package in the cache.
pub fn download_package(
    repository: &str,
    package: &RepositoryPackage,
    destination: &Path,
) -> Result<()> {
    if repository.trim().is_empty() {
        bail!("repository URL/path cannot be empty");
    }

    validate_package_for_download(package)?;

    ensure_destination_parent(destination)?;

    let temporary = temporary_download_path(destination);

    remove_if_exists(&temporary)?;

    let result = if is_http_repository(repository) {
        download_remote_package(repository, package, &temporary)
    } else {
        copy_local_package(repository, package, &temporary)
    };

    if let Err(error) = result {
        remove_if_exists(&temporary)?;
        return Err(error);
    }

    verify_downloaded_package(&temporary, package)?;

    replace_destination(&temporary, destination)?;

    Ok(())
}

/// Validate fields needed to download a package.
fn validate_package_for_download(package: &RepositoryPackage) -> Result<()> {
    if package.name.trim().is_empty() {
        bail!("cannot download package with empty name");
    }

    if package.filename.trim().is_empty() {
        bail!("package `{}` has empty filename", package.name);
    }

    if package.sha256.trim().is_empty() {
        bail!("package `{}` has no SHA-256 checksum", package.name);
    }

    if !is_valid_sha256(package.sha256.trim()) {
        bail!("package `{}` has invalid SHA-256 checksum", package.name);
    }

    if package.download_url.trim().is_empty() {
        bail!("package `{}` has empty download URL", package.name);
    }

    Ok(())
}

/// Download a package through HTTP(S).
fn download_remote_package(
    repository: &str,
    package: &RepositoryPackage,
    temporary: &Path,
) -> Result<()> {
    let package_url = resolve_package_url(repository, package.download_url.trim())?;

    println!("Downloading from: {}", package_url);

    let client = build_http_client(DOWNLOAD_TIMEOUT)?;

    let response = client
        .get(package_url.clone())
        .send()
        .with_context(|| format!("failed to download package from {package_url}"))?;

    let response = ensure_success(response, &package_url)?;

    write_http_response_to_file(response, temporary).with_context(|| {
        format!(
            "failed to save package `{}` to {}",
            package.name,
            temporary.display()
        )
    })?;

    Ok(())
}

/// Copy a package from a local repository.
fn copy_local_package(
    repository: &str,
    package: &RepositoryPackage,
    temporary: &Path,
) -> Result<()> {
    let base = PathBuf::from(repository);

    if !base.exists() {
        bail!("local repository does not exist: {}", base.display());
    }

    if !base.is_dir() {
        bail!("local repository is not a directory: {}", base.display());
    }

    let packages_dir = base.join(LOCAL_PACKAGES_DIR);

    if !packages_dir.is_dir() {
        bail!(
            "local repository package directory does not exist: {}",
            packages_dir.display()
        );
    }

    let source = packages_dir.join(&package.filename);

    if !is_safe_relative_filename(&package.filename) {
        bail!("unsafe package filename `{}`", package.filename);
    }

    if !source.exists() {
        bail!(
            "package `{}` not found in local repository: {}",
            package.name,
            source.display()
        );
    }

    if !source.is_file() {
        bail!(
            "package `{}` is not a regular file: {}",
            package.name,
            source.display()
        );
    }

    fs::copy(&source, temporary).with_context(|| {
        format!(
            "failed to copy {} to {}",
            source.display(),
            temporary.display()
        )
    })?;

    Ok(())
}

/// Stream an HTTP response into a file.
fn write_http_response_to_file(mut response: Response, destination: &Path) -> Result<()> {
    let mut file = File::create(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;

    copy(&mut response, &mut file).with_context(|| {
        format!(
            "failed to write downloaded data to {}",
            destination.display()
        )
    })?;

    Ok(())
}

/// Verify a downloaded package against the repository checksum.
fn verify_downloaded_package(path: &Path, package: &RepositoryPackage) -> Result<()> {
    if !path.exists() {
        bail!("downloaded package does not exist: {}", path.display());
    }

    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to inspect downloaded package {}", path.display()))?;

    if !metadata.is_file() {
        bail!(
            "downloaded package is not a regular file: {}",
            path.display()
        );
    }

    if metadata.len() == 0 {
        bail!("downloaded package is empty: {}", path.display());
    }

    let actual = sha256_file(path)
        .with_context(|| format!("failed to calculate SHA-256 for {}", path.display()))?;

    let expected = package.sha256.trim();

    if !actual.eq_ignore_ascii_case(expected) {
        bail!(
            "SHA-256 mismatch for {}: expected {}, got {}",
            package.filename,
            expected,
            actual
        );
    }

    println!("SHA-256 verified for {}: {}", package.filename, actual);

    Ok(())
}

/// Create a temporary filename next to the final destination.
fn temporary_download_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package");

    destination.with_file_name(format!(".{file_name}.download"))
}

/// Ensure destination directory exists.
fn ensure_destination_parent(destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create destination directory {}",
                parent.display()
            )
        })?;
    }

    Ok(())
}

/// Remove a file if it exists.
fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

/// Replace the final destination with the completed temporary file.
fn replace_destination(temporary: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        remove_if_exists(destination)?;
    }

    fs::rename(temporary, destination).with_context(|| {
        format!(
            "failed to move downloaded package {} to {}",
            temporary.display(),
            destination.display()
        )
    })?;

    Ok(())
}

// ============================================================================
// Package lookup
// ============================================================================

/// Find a package by an exact normalized name.
///
/// This is deliberately more forgiving than the original implementation:
///
///     util-linux
///     UTIL-LINUX
///     Util_Linux
///
/// all resolve to the same normalized package name.
///
/// Exact normalized matches are always preferred.
pub fn find_package<'a>(index: &'a RepositoryIndex, name: &str) -> Option<&'a RepositoryPackage> {
    let query = normalize_search_text(name);

    if query.is_empty() {
        return None;
    }

    index
        .packages
        .iter()
        .find(|package| normalize_package_name(&package.name) == query)
}

/// Search packages by name or description.
///
/// Matching supports:
///
/// - case-insensitive text
/// - hyphens
/// - underscores
/// - periods
/// - plus signs
/// - spaces
/// - repeated separators
///
/// Search ranking:
///
/// 1. exact normalized name
/// 2. name starts with query
/// 3. name contains query
/// 4. description contains query
///
/// This means `util-linux` is handled as a normal package name, not as a
/// command-line operator or special syntax.
pub fn search<'a>(index: &'a RepositoryIndex, query: &str) -> Vec<&'a RepositoryPackage> {
    let normalized_query = normalize_search_text(query);

    if normalized_query.is_empty() {
        return Vec::new();
    }

    let query_tokens = tokenize_search_text(query);

    let mut results: Vec<(&RepositoryPackage, SearchScore)> = index
        .packages
        .iter()
        .filter_map(|package| {
            let name_normalized = normalize_search_text(&package.name);
            let description_normalized = normalize_search_text(&package.description);

            let name_compact = compact_search_text(&package.name);
            let description_compact = compact_search_text(&package.description);

            let mut score = SearchScore::default();

            if name_normalized == normalized_query {
                score.exact_name = true;
            }

            if name_normalized.starts_with(&normalized_query) {
                score.name_prefix = true;
            }

            if name_normalized.contains(&normalized_query) {
                score.name_contains = true;
            }

            if description_normalized.contains(&normalized_query) {
                score.description_contains = true;
            }

            if !query_tokens.is_empty()
                && query_tokens.iter().all(|token| {
                    name_normalized.contains(token) || description_normalized.contains(token)
                })
            {
                score.all_tokens_match = true;
            }

            // Compact comparison makes variations such as:
            //
            //     util-linux
            //     util_linux
            //     util linux
            //
            // behave similarly.
            let compact_query = compact_search_text(query);

            if !compact_query.is_empty() {
                if name_compact == compact_query {
                    score.compact_exact = true;
                }

                if name_compact.contains(&compact_query) {
                    score.compact_contains = true;
                }

                if description_compact.contains(&compact_query) {
                    score.compact_description = true;
                }
            }

            if score.matches() {
                Some((package, score))
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.name.to_lowercase().cmp(&right.0.name.to_lowercase()))
    });

    results
        .into_iter()
        .map(|(package, _score)| package)
        .collect()
}

// ============================================================================
// Search normalization
// ============================================================================

/// Normalize a package name.
///
/// Package names intentionally remain broadly permissive. We do not reject
/// punctuation because package managers commonly have names such as:
///
///     foo-bar
///     foo_bar
///     foo+bar
///     foo.bar
///     foo@bar
///
/// The normalization is only used for matching, never for writing package
/// names back to disk.
fn normalize_package_name(name: &str) -> String {
    normalize_search_text(name)
}

/// Normalize text for search.
///
/// Unicode-aware lowercase is used where possible. Separators are converted
/// into a single ASCII space so punctuation differences do not prevent useful
/// matches.
///
/// Examples:
///
///     "util-linux"      -> "util linux"
///     "UTIL_LINUX"      -> "util linux"
///     "util..linux"     -> "util linux"
///     "util   linux"    -> "util linux"
fn normalize_search_text(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut previous_was_separator = false;

    for character in input.chars().flat_map(char::to_lowercase) {
        let is_alphanumeric = character.is_alphanumeric();

        if is_alphanumeric {
            output.push(character);
            previous_was_separator = false;
            continue;
        }

        if !previous_was_separator {
            output.push(' ');
            previous_was_separator = true;
        }
    }

    output.trim().to_string()
}

/// Remove separators entirely.
///
/// This makes matching tolerant of:
///
///     foo-bar
///     foo_bar
///     foo bar
///     foo.bar
///
/// without changing the original package name.
fn compact_search_text(input: &str) -> String {
    input
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

/// Split normalized search text into useful tokens.
fn tokenize_search_text(input: &str) -> Vec<String> {
    normalize_search_text(input)
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

// ============================================================================
// Search scoring
// ============================================================================

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
struct SearchScore {
    exact_name: bool,
    compact_exact: bool,
    name_prefix: bool,
    compact_contains: bool,
    name_contains: bool,
    all_tokens_match: bool,
    description_contains: bool,
    compact_description: bool,
}

impl SearchScore {
    fn matches(self) -> bool {
        self.exact_name
            || self.compact_exact
            || self.name_prefix
            || self.compact_contains
            || self.name_contains
            || self.all_tokens_match
            || self.description_contains
            || self.compact_description
    }

    fn rank(self) -> u32 {
        let mut rank = 0;

        if self.exact_name {
            rank += 10_000;
        }

        if self.compact_exact {
            rank += 8_000;
        }

        if self.name_prefix {
            rank += 6_000;
        }

        if self.compact_contains {
            rank += 5_000;
        }

        if self.name_contains {
            rank += 4_000;
        }

        if self.all_tokens_match {
            rank += 2_000;
        }

        if self.description_contains {
            rank += 1_000;
        }

        if self.compact_description {
            rank += 500;
        }

        rank
    }
}

impl Ord for SearchScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl PartialOrd for SearchScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ============================================================================
// Validation helpers
// ============================================================================

/// Determine whether a download URL is HTTP(S).
fn is_http_download_url(url: &str) -> bool {
    let trimmed = url.trim();

    let lower = trimmed.to_ascii_lowercase();

    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Validate a SHA-256 string.
///
/// A SHA-256 digest is exactly 64 hexadecimal characters.
fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|character| character.is_ascii_hexdigit())
}

/// Ensure a local package filename cannot escape the repository's packages
/// directory through path traversal.
///
/// Valid:
///
///     foo.boob
///     foo-bar-1.0-x86_64.boob
///     nested/foo.boob
///
/// Rejected:
///
///     ../foo.boob
///     /absolute/path
///     ../../secret
///     C:\foo.boob
///
/// The filename can still contain normal punctuation such as `-`, `_`, `.`,
/// `+`, and `@`.
fn is_safe_relative_filename(filename: &str) -> bool {
    let path = Path::new(filename);

    if filename.trim().is_empty() {
        return false;
    }

    if path.is_absolute() {
        return false;
    }

    if filename.contains('\0') {
        return false;
    }

    for component in path.components() {
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,

            Component::Normal(_) | Component::CurDir => {}
        }
    }

    true
}

// ============================================================================
// Compatibility / diagnostic helpers
// ============================================================================

/// Return a human-readable description of the repository source.
pub fn repository_kind(repository: &str) -> &'static str {
    if is_http_repository(repository) {
        "remote HTTP(S)"
    } else {
        "local filesystem"
    }
}

/// Validate repository configuration without downloading anything.
///
/// This is useful for future diagnostics and makes repository validation
/// reusable by CLI commands.
pub fn validate_repository(repository: &str) -> Result<()> {
    let repository = repository.trim();

    if repository.is_empty() {
        bail!("repository URL/path cannot be empty");
    }

    if is_http_repository(repository) {
        let _ = parse_repository_url(repository)?;
        Ok(())
    } else {
        let path = Path::new(repository);

        if !path.exists() {
            bail!("local repository does not exist: {}", path.display());
        }

        if !path.is_dir() {
            bail!("local repository is not a directory: {}", path.display());
        }

        let index = path.join(INDEX_FILE_NAME);

        if !index.exists() {
            bail!(
                "local repository database does not exist: {}",
                index.display()
            );
        }

        if !index.is_file() {
            bail!(
                "local repository database is not a regular file: {}",
                index.display()
            );
        }

        Ok(())
    }
}

/// Return a normalized representation useful for diagnostics.
pub fn normalize_query(query: &str) -> String {
    normalize_search_text(query)
}

/// Return the repository index filename.
pub fn index_filename() -> &'static str {
    INDEX_FILE_NAME
}

/// Return the package storage directory used by local repositories.
pub fn local_packages_directory() -> &'static str {
    LOCAL_PACKAGES_DIR
}

// ============================================================================
// Future-proof file inspection helpers
// ============================================================================

/// Read a file completely.
///
/// Kept private for now but intentionally implemented independently from the
/// JSON/database layer. This gives us a convenient place for future package
/// metadata or repository signature support.
fn read_file_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;

    let mut reader = BufReader::new(file);
    let mut data = Vec::new();

    reader
        .read_to_end(&mut data)
        .with_context(|| format!("failed to read {}", path.display()))?;

    Ok(data)
}

/// Check whether a local repository index can be read.
///
/// This intentionally does not deserialize it; it only verifies that the file
/// exists and contains readable bytes.
fn repository_index_readable(path: &Path) -> Result<()> {
    let _ = read_file_bytes(path)?;
    Ok(())
}

// ============================================================================
// End of repository module
// ============================================================================
