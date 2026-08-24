use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::{Archive, EntryType};

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

        let entry_path = entry.path()?.to_path_buf();
        let entry_type = entry.header().entry_type();

        // metadata.json fica fora de root/
        if entry_path == Path::new("metadata.json") {
            if !entry_type.is_file() {
                anyhow::bail!("metadata.json is not a regular file");
            }

            let mut text = String::new();

            entry
                .read_to_string(&mut text)
                .context("failed to read metadata.json")?;

            metadata = Some(
                serde_json::from_str::<PackageMetadata>(&text)
                    .context("failed to parse metadata.json")?,
            );

            continue;
        }

        // Tudo além de metadata.json precisa estar dentro de root/
        let relative_path = entry_path
            .strip_prefix("root")
            .with_context(|| format!("package member `{}` is outside root/", entry_path.display()))?
            .to_path_buf();

        // A entrada root/ em si é um diretório vazio e não precisa ser instalada.
        if relative_path.as_os_str().is_empty() {
            if entry_type.is_dir() {
                continue;
            }

            anyhow::bail!("package member `{}` is invalid", entry_path.display());
        }

        validate_relative_path(&relative_path)?;

        // DIRETÓRIOS:
        //
        // Não adicionamos diretórios como PackageEntry porque o instalador
        // deve criar automaticamente os diretórios pais dos arquivos.
        //
        // Isso evita tentar escrever um diretório como /opt como se fosse
        // um arquivo.
        if entry_type.is_dir() {
            continue;
        }

        // Por enquanto aceitamos somente arquivos regulares.
        //
        // Isso também evita que symlinks, hardlinks e outros tipos especiais
        // dentro do pacote possam escapar ou causar comportamento inesperado.
        if !entry_type.is_file() {
            anyhow::bail!(
                "unsupported package member type for `{}`",
                entry_path.display()
            );
        }

        let mut data = Vec::new();

        entry
            .read_to_end(&mut data)
            .with_context(|| format!("failed to read package member `{}`", entry_path.display()))?;

        let mode = entry.header().mode().ok();

        members.push(PackageEntry {
            relative_path,
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
