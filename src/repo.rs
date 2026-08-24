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

        let client = Client::builder().user_agent("boobies/0.1.0").build()?;

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
    // Para repositórios HTTP(S), use diretamente a URL definida
    // no database.json. Isso permite baixar pacotes de GitHub Releases,
    // GitLab, CDN etc. sem montar "/packages/..." automaticamente.
    if repository.starts_with("http://") || repository.starts_with("https://") {
        let url = &package.download_url;

        println!("Downloading from: {}", url);

        let client = Client::builder()
            .user_agent("boobies/0.1.0")
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()?;

        let mut response = client
            .get(url)
            .send()
            .with_context(|| format!("failed to download {url}"))?
            .error_for_status()
            .with_context(|| format!("repository returned an error for {url}"))?;

        // Garante que a pasta do cache exista.
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create cache directory {}", parent.display())
            })?;
        }

        let mut file = File::create(destination)
            .with_context(|| format!("failed to create {}", destination.display()))?;

        copy(&mut response, &mut file).with_context(|| {
            format!(
                "failed to save downloaded package to {}",
                destination.display()
            )
        })?;
    } else {
        // Suporte para repositório local.
        let source = PathBuf::from(repository)
            .join("packages")
            .join(&package.filename);

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create cache directory {}", parent.display())
            })?;
        }

        std::fs::copy(&source, destination).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source.display(),
                destination.display()
            )
        })?;
    }

    // Confere a integridade do arquivo baixado.
    let actual = sha256_file(destination)
        .with_context(|| format!("failed to calculate SHA-256 for {}", destination.display()))?;

    let actual = sha256_file(destination)?;

    println!(
        "EXPECTED: {:?} len={}",
        package.sha256,
        package.sha256.len()
    );
    println!("ACTUAL:   {:?} len={}", actual, actual.len());

    if actual.trim().ne(package.sha256.trim()) {
        anyhow::bail!(
            "SHA-256 mismatch for {}: expected {}, got {}",
            package.filename,
            package.sha256,
            actual
        );
    }

    Ok(())
}

pub fn find_package<'a>(index: &'a RepositoryIndex, name: &str) -> Option<&'a RepositoryPackage> {
    index.packages.iter().find(|pkg| pkg.name == name)
}

pub fn search<'a>(index: &'a RepositoryIndex, query: &str) -> Vec<&'a RepositoryPackage> {
    let q = query.to_lowercase();

    index
        .packages
        .iter()
        .filter(|pkg| {
            pkg.name.to_lowercase().contains(&q) || pkg.description.to_lowercase().contains(&q)
        })
        .collect()
}
