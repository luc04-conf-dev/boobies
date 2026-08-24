use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(default)]
    pub requirement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryPackage {
    pub name: String,
    pub version: String,
    pub architecture: String,
    #[serde(default)]
    pub description: String,
    pub filename: String,
    pub sha256: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepositoryIndex {
    pub format_version: u32,
    pub generated_at: Option<String>,
    pub packages: Vec<RepositoryPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub metadata: PackageMetadata,
    pub installed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledDatabase {
    pub format_version: u32,
    pub packages: BTreeMap<String, InstalledPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub repository: String,
    pub root: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repository: "https://example.invalid/boobies/".to_string(),
            root: PathBuf::from("/"),
        }
    }
}
