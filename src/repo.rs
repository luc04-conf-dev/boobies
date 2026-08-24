use std::{
    fs::File,
    io::copy,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use url::Url;

use crate::{
    database,
    models::{RepositoryIndex, RepositoryPackage},
    package::sha256_file,
};

pub fn normalize_base(base: &str) -> Result<String> {
    let trimmed = base.trim_end_matches('/');
    if trimmed.is_empty() {
        anyhow::bail!("repository URL/path cannot be empty");
    }
    Ok(format!("{trimmed}/"))
}

pub fn fetch_repository_index(repository: &str) -> Result<RepositoryIndex> {
    if repository.starts_with("http://") || repository.starts_with("https://") {
        let url = format!("{}database.json", normalize_base(repository)?);

        let client = Client::builder()
            .user_agent("boobies/0.1.0")
            .build()?;

        let response = client
            .get(&url)
            .send()
            .with_context(|| format!("failed to GET {url}"))?
            .error_for_status()
            .with_context(|| format!("repository returned an error for {url}"))?;

        Ok(response.json()?)
    } else {
        let path = PathBuf::from(repository).join("database.json");
        database::load_json(&path)
    }
}

pub fn download_package(
    repository: &str,
    package: &RepositoryPackage,
    destination: &Path,
) -> Result<()> {
    if repository.starts_with("http://") || repository.starts_with("https://") {
        let base = normalize_base(repository)?;
        let url = Url::parse(&base)?
            .join(&format!("packages/{}", package.filename))?
            .to_string();

        let client = Client::builder()
            .user_agent("boobies/0.1.0")
            .build()?;

        let mut response = client
            .get(&url)
            .send()
            .with_context(|| format!("failed to download {url}"))?
            .error_for_status()
            .with_context(|| format!("repository returned an error for {url}"))?;

        let mut file = File::create(destination)?;
        copy(&mut response, &mut file)?;
    } else {
        let source = PathBuf::from(repository)
            .join("packages")
            .join(&package.filename);

        std::fs::copy(&source, destination)
            .with_context(|| format!("failed to copy {}", source.display()))?;
    }

    let actual = sha256_file(destination)?;
    if !actual.eq_ignore_ascii_case(&package.sha256) {
        anyhow::bail!(
            "SHA-256 mismatch for {}: expected {}, got {}",
            package.filename,
            package.sha256,
            actual
        );
    }

    Ok(())
}

pub fn find_package<'a>(
    index: &'a RepositoryIndex,
    name: &str,
) -> Option<&'a RepositoryPackage> {
    index.packages.iter().find(|pkg| pkg.name == name)
}

pub fn search<'a>(index: &'a RepositoryIndex, query: &str) -> Vec<&'a RepositoryPackage> {
    let q = query.to_lowercase();

    index
        .packages
        .iter()
        .filter(|pkg| {
            pkg.name.to_lowercase().contains(&q)
                || pkg.description.to_lowercase().contains(&q)
        })
        .collect()
}
