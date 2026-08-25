use std::{
    env,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{de::DeserializeOwned, Serialize};

use crate::models::{Config, InstalledDatabase, RepositoryIndex};

/// Return the home directory of the user who invoked sudo.
///
/// Example:
///
///     sudo boobies bigger btop
///
/// In that case:
/// - SUDO_USER = original username
/// - HOME may be /root
///
/// We intentionally ignore /root and resolve the original user's home.
fn sudo_user_home_dir() -> Option<PathBuf> {
    let username = env::var("SUDO_USER").ok()?;

    let passwd = fs::read_to_string("/etc/passwd").ok()?;

    passwd.lines().find_map(|line| {
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        let fields: Vec<&str> = line.split(':').collect();

        // /etc/passwd:
        // username:x:uid:gid:gecos:home:shell
        if fields.len() >= 6 && fields[0] == username {
            Some(PathBuf::from(fields[5]))
        } else {
            None
        }
    })
}

/// Return the directory used for Boobies user configuration.
///
/// Normal execution:
///
///     ~/.config/boobies
///
/// sudo execution:
///
///     /home/<original-user>/.config/boobies
///
/// Never:
///
///     /root/.config/boobies
pub fn config_dir(custom: Option<&Path>) -> PathBuf {
    // Explicit configuration directory always wins.
    if let Some(path) = custom {
        return path.to_path_buf();
    }

    // When running through sudo, use the original user's home.
    if env::var_os("SUDO_USER").is_some() {
        if let Some(home) = sudo_user_home_dir() {
            return home.join(".config").join("boobies");
        }
    }

    // Normal non-sudo execution.
    if let Some(home) = home_dir() {
        return home.join(".config").join("boobies");
    }

    // Last-resort fallback.
    PathBuf::from(".boobies")
}

pub fn config_path(custom: Option<&Path>) -> PathBuf {
    config_dir(custom).join("config.json")
}

pub fn repository_cache_path(custom: Option<&Path>) -> PathBuf {
    config_dir(custom).join("repository.json")
}

/// Database of packages installed by Boobies.
///
/// System installation:
///
///     /var/lib/boobies/installed.json
///
/// Non-system root:
///
///     <root>/var/lib/boobies/installed.json
///
/// The user's config directory is deliberately NOT used here.
pub fn installed_db_path(root: &Path, _custom: Option<&Path>) -> PathBuf {
    if root == Path::new("/") {
        PathBuf::from("/var/lib/boobies/installed.json")
    } else {
        root.join("var/lib/boobies/installed.json")
    }
}

pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse JSON {}", path.display()))?;

    Ok(value)
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let data = serde_json::to_vec_pretty(value)?;
    let tmp = path.with_extension("tmp");

    {
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("failed to create {}", tmp.display()))?;

        file.write_all(&data)?;
        file.sync_all()?;
    }

    fs::rename(&tmp, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;

    Ok(())
}

pub fn load_config(custom: Option<&Path>, root: &Path) -> Result<Config> {
    let path = config_path(custom);

    if path.exists() {
        let mut config: Config = load_json(&path)?;

        // A custom/non-system root overrides the root stored in config.json.
        if root != Path::new("/") {
            config.root = root.to_path_buf();
        }

        return Ok(config);
    }

    let mut config = Config::default();
    config.root = root.to_path_buf();

    save_json(&path, &config)?;

    Ok(config)
}

pub fn load_installed_db(
    root: &Path,
    custom: Option<&Path>,
) -> Result<InstalledDatabase> {
    let path = installed_db_path(root, custom);

    if !path.exists() {
        return Ok(InstalledDatabase {
            format_version: 1,
            packages: Default::default(),
        });
    }

    load_json(&path)
}

pub fn save_installed_db(
    root: &Path,
    custom: Option<&Path>,
    db: &InstalledDatabase,
) -> Result<()> {
    let path = installed_db_path(root, custom);

    save_json(&path, db)
}

pub fn load_repository_cache(
    custom: Option<&Path>,
) -> Result<RepositoryIndex> {
    let path = repository_cache_path(custom);

    if !path.exists() {
        anyhow::bail!(
            "local repository database not found; run `boobies grow` first"
        );
    }

    load_json(&path)
}