use std::{
    cmp::Ordering,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    cli::{Cli, Command},
    database, installer,
    models::{Config, Dependency, InstalledDatabase, RepositoryPackage},
    package::{package_architecture, read_package},
    repo,
};

pub fn dispatch(cli: Cli) -> Result<()> {
    if let Some(query) = cli.search.as_deref() {
        let config = database::load_config(cli.config_dir.as_deref(), &cli.root)?;
        let index = database::load_repository_cache(cli.config_dir.as_deref())?;
        let matches = repo::search(&index, query);

        println!("Searching the Valle of Boobies for `{query}`...");
        if matches.is_empty() {
            println!("No packages found.");
            return Ok(());
        }

        for pkg in matches {
            println!(
                "{:<24} {:<12} {:<10} {}",
                pkg.name, pkg.version, pkg.architecture, pkg.description
            );
        }

        let _ = config;
        return Ok(());
    }

    if let Some(package) = cli.info_flag.as_deref() {
        let _config = database::load_config(cli.config_dir.as_deref(), &cli.root)?;
        let index = database::load_repository_cache(cli.config_dir.as_deref())?;
        let pkg = repo::find_package(&index, package)
            .with_context(|| format!("package `{package}` not found"))?;

        print_repo_info(pkg);
        return Ok(());
    }

    match cli.command.as_ref() {
        Some(Command::Bigger { package, yes }) => bigger(&cli, package, *yes),
        Some(Command::Smaller { package, yes }) => smaller(&cli, package, *yes),
        Some(Command::Grow { force: _ }) => grow(&cli),
        Some(Command::Expand { yes }) => expand(&cli, *yes),
        Some(Command::List) => list(&cli),
        Some(Command::Version) => {
            println!("boobies {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        None => unreachable!("clap should reject missing command"),
    }
}

fn bigger(cli: &Cli, package: &str, yes: bool) -> Result<()> {
    let config = database::load_config(cli.config_dir.as_deref(), &cli.root)?;
    let mut db = database::load_installed_db(&cli.root, cli.config_dir.as_deref())?;

    let package_path = PathBuf::from(package);

    let local_path = if package_path.exists() {
        package_path
    } else {
        let index = database::load_repository_cache(cli.config_dir.as_deref())?;
        let repo_pkg = repo::find_package(&index, package)
            .with_context(|| format!("package `{package}` not found in repository"))?;

        check_architecture(repo_pkg.architecture.as_str())?;

        let cache_dir = database::config_dir(cli.config_dir.as_deref()).join("cache");
        fs::create_dir_all(&cache_dir)?;

        let destination = cache_dir.join(&repo_pkg.filename);

        println!(
            "Boobies is getting bigger: {} {}",
            repo_pkg.name, repo_pkg.version
        );
        repo::download_package(&config.repository, repo_pkg, &destination)?;

        destination
    };

    let preview = read_package(&local_path)?;

    check_architecture(&preview.metadata.architecture)?;

    if !yes {
        println!(
            "Install {} {} with {} files into {}?",
            preview.metadata.name,
            preview.metadata.version,
            preview.members.len(),
            cli.root.display()
        );

        if !confirm()? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    ensure_not_breaking_existing(&preview.metadata.dependencies, &db)?;

    let installed = installer::install_package(&cli.root, &local_path, &mut db)?;
    database::save_installed_db(&cli.root, cli.config_dir.as_deref(), &db)?;

    println!(
        "Installed {} {}. The system got bigger. 💀",
        installed.metadata.name, installed.metadata.version
    );

    Ok(())
}

fn smaller(cli: &Cli, package: &str, yes: bool) -> Result<()> {
    let mut db = database::load_installed_db(&cli.root, cli.config_dir.as_deref())?;

    let installed = db
        .packages
        .get(package)
        .with_context(|| format!("package `{package}` is not installed"))?;

    if !yes {
        println!(
            "Remove {} {}?",
            installed.metadata.name, installed.metadata.version
        );

        if !confirm()? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let removed = installer::remove_package(&cli.root, package, &mut db)?;
    database::save_installed_db(&cli.root, cli.config_dir.as_deref(), &db)?;

    println!(
        "Removed {} {}. The system got smaller.",
        removed.metadata.name, removed.metadata.version
    );

    Ok(())
}

fn grow(cli: &Cli) -> Result<()> {
    let config = database::load_config(cli.config_dir.as_deref(), &cli.root)?;
    let index = repo::fetch_repository_index(&config.repository)?;

    database::save_json(
        &database::repository_cache_path(cli.config_dir.as_deref()),
        &index,
    )?;

    println!(
        "The Valle has grown. {} packages are now known.",
        index.packages.len()
    );

    Ok(())
}

fn expand(cli: &Cli, yes: bool) -> Result<()> {
    let index = database::load_repository_cache(cli.config_dir.as_deref())?;
    let db = database::load_installed_db(&cli.root, cli.config_dir.as_deref())?;

    let mut upgrades: Vec<(&str, &RepositoryPackage)> = Vec::new();

    for (name, installed) in &db.packages {
        if let Some(candidate) = repo::find_package(&index, name) {
            if version_cmp(&candidate.version, &installed.metadata.version) == Ordering::Greater {
                upgrades.push((name.as_str(), candidate));
            }
        }
    }

    if upgrades.is_empty() {
        println!("Nothing to expand. Everything is already maximally boobied.");
        return Ok(());
    }

    println!("Packages available for expansion:");
    for (name, candidate) in &upgrades {
        println!("  {name}: {}", candidate.version);
    }

    if !yes {
        print!("Expand everything listed above? [y/N] ");
        io::stdout().flush()?;
        if !confirm()? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    for (name, candidate) in upgrades {
        println!("Expanding {name} → {}...", candidate.version);

        let cache_dir = database::config_dir(cli.config_dir.as_deref()).join("cache");
        fs::create_dir_all(&cache_dir)?;
        let destination = cache_dir.join(&candidate.filename);

        let config = database::load_config(cli.config_dir.as_deref(), &cli.root)?;
        repo::download_package(&config.repository, candidate, &destination)?;

        let mut db = database::load_installed_db(&cli.root, cli.config_dir.as_deref())?;
        installer::remove_package(&cli.root, name, &mut db)?;
        installer::install_package(&cli.root, &destination, &mut db)?;
        database::save_installed_db(&cli.root, cli.config_dir.as_deref(), &db)?;
    }

    println!("Expansion complete.");
    Ok(())
}

fn list(cli: &Cli) -> Result<()> {
    let db = database::load_installed_db(&cli.root, cli.config_dir.as_deref())?;

    if db.packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!("Installed packages:");
    for (name, package) in db.packages {
        println!(
            "{:<24} {:<16} {}",
            name, package.metadata.version, package.metadata.architecture
        );
    }

    Ok(())
}

fn print_repo_info(pkg: &RepositoryPackage) {
    println!("Name:         {}", pkg.name);
    println!("Version:      {}", pkg.version);
    println!("Architecture: {}", pkg.architecture);
    println!("Description:  {}", pkg.description);
    println!("Filename:     {}", pkg.filename);
    println!("SHA-256:      {}", pkg.sha256);

    if pkg.dependencies.is_empty() {
        println!("Dependencies: none");
    } else {
        println!("Dependencies:");
        for dep in &pkg.dependencies {
            let req = dep.requirement.as_deref().unwrap_or("*");
            println!("  - {} {}", dep.name, req);
        }
    }
}

fn check_architecture(architecture: &str) -> Result<()> {
    let local = package_architecture();

    if architecture.is_empty() || architecture == "any" || architecture == local {
        return Ok(());
    }

    anyhow::bail!(
        "package architecture `{architecture}` does not match local architecture `{local}`"
    );
}

fn ensure_not_breaking_existing(dependencies: &[Dependency], db: &InstalledDatabase) -> Result<()> {
    for dep in dependencies {
        if !db.packages.contains_key(&dep.name) {
            anyhow::bail!(
                "missing dependency: {} {}",
                dep.name,
                dep.requirement.as_deref().unwrap_or("")
            );
        }
    }

    Ok(())
}

fn confirm() -> Result<bool> {
    print!("Proceed? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn version_cmp(a: &str, b: &str) -> Ordering {
    let split = |s: &str| {
        s.split(['.', '-', '_'])
            .map(|part| {
                part.parse::<u64>()
                    .map(|n| (0u8, n.to_string()))
                    .unwrap_or((1u8, part.to_ascii_lowercase()))
            })
            .collect::<Vec<_>>()
    };

    let aa = split(a);
    let bb = split(b);
    aa.cmp(&bb)
}

#[allow(dead_code)]
fn ensure_root_exists(root: &Path) -> Result<()> {
    if !root.exists() {
        fs::create_dir_all(root)?;
    }
    Ok(())
}
