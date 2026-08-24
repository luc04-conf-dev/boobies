use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

use crate::models::PackageMetadata;

pub struct OpenPackage {
    pub metadata: PackageMetadata,
    pub members: Vec<PackageEntry>,
}

pub struct PackageEntry {
    pub relative_path: PathBuf,
    pub data: Vec<u8>,
    pub mode: Option<u32>,
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn read_package(path: &Path) -> Result<OpenPackage> {
    let file =
        File::open(path).with_context(|| format!("failed to open package {}", path.display()))?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    let mut metadata = None;
    let mut members = Vec::new();

    for item in archive.entries()? {
        let mut entry = item?;
        let path = entry.path()?.to_path_buf();

        if path == Path::new("metadata.json") {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            metadata = Some(serde_json::from_str::<PackageMetadata>(&text)?);
            continue;
        }

        let path = if let Ok(stripped) = path.strip_prefix("root") {
            stripped.to_path_buf()
        } else {
            anyhow::bail!("package member `{}` is outside root/", path.display());
        };

        validate_relative_path(&path)?;

        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;

        let mode = entry.header().mode().ok();

        members.push(PackageEntry {
            relative_path: path,
            data,
            mode,
        });
    }

    let metadata = metadata.context("package is missing metadata.json")?;

    if metadata.name.trim().is_empty() {
        anyhow::bail!("package name cannot be empty");
    }

    if metadata.version.trim().is_empty() {
        anyhow::bail!("package version cannot be empty");
    }

    Ok(OpenPackage { metadata, members })
}

pub fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() || path == Path::new(".") {
        anyhow::bail!("empty/invalid package path");
    }

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                anyhow::bail!("unsafe package path `{}`", path.display());
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

pub fn package_architecture() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    }
}

pub fn safe_join(root: &Path, relative: &Path) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    Ok(root.join(relative))
}

pub fn ensure_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(())
}
