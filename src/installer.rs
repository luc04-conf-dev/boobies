use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    models::{InstalledDatabase, InstalledPackage},
    package::{ensure_directory, read_package, safe_join},
};

pub fn install_package(
    root: &Path,
    package_path: &Path,
    db: &mut InstalledDatabase,
) -> Result<InstalledPackage> {
    let package = read_package(package_path)?;

    if let Some(existing) = db.packages.get(&package.metadata.name) {
        if existing.metadata.version == package.metadata.version {
            anyhow::bail!(
                "{} {} is already installed",
                package.metadata.name,
                package.metadata.version
            );
        }
    }

    let mut installed_files = Vec::new();

    for member in &package.members {
        let destination = safe_join(root, &member.relative_path)?;

        if let Some(parent) = destination.parent() {
            ensure_directory(parent)?;
        }

        if destination.exists() && destination.is_dir() {
            anyhow::bail!(
                "cannot overwrite directory with file: {}",
                destination.display()
            );
        }

        fs::write(&destination, &member.data)
            .with_context(|| format!("failed to install {}", destination.display()))?;

        if let Some(mode) = member.mode {
            set_mode(&destination, mode)?;
        }

        installed_files.push(member.relative_path.clone());
    }

    let installed = InstalledPackage {
        metadata: package.metadata,
        installed_files,
    };

    db.packages
        .insert(installed.metadata.name.clone(), installed.clone());

    Ok(installed)
}

pub fn remove_package(
    root: &Path,
    package_name: &str,
    db: &mut InstalledDatabase,
) -> Result<InstalledPackage> {
    let installed = db
        .packages
        .get(package_name)
        .cloned()
        .with_context(|| format!("package `{package_name}` is not installed"))?;

    let protected: HashSet<PathBuf> = db
        .packages
        .iter()
        .filter(|(name, _)| name.as_str() != package_name)
        .flat_map(|(_, pkg)| pkg.installed_files.iter().cloned())
        .collect();

    for relative in installed.installed_files.iter().rev() {
        if protected.contains(relative) {
            continue;
        }

        let destination = safe_join(root, relative)?;

        if destination.is_file() || destination.is_symlink() {
            fs::remove_file(&destination)
                .with_context(|| format!("failed to remove {}", destination.display()))?;

            clean_empty_parents(root, destination.parent());
        }
    }

    db.packages.remove(package_name);

    Ok(installed)
}

fn clean_empty_parents(root: &Path, mut dir: Option<&Path>) {
    while let Some(current) = dir {
        if current == root || current.as_os_str().is_empty() {
            break;
        }

        let is_empty = match fs::read_dir(current) {
            Ok(mut entries) => entries.next().is_none(),
            Err(_) => false,
        };

        if !is_empty {
            break;
        }

        if fs::remove_dir(current).is_err() {
            break;
        }

        dir = current.parent();
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o7777))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
