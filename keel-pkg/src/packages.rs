use anyhow::{Context, Result};
use console::style;
use fs_err::{File, OpenOptions};
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::io::{Seek, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, io::Read, path::Path};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct PackagesManifest {
    pub version: String,
    pub packages: HashMap<String, Package>,
}

impl Default for PackagesManifest {
    fn default() -> Self {
        PackagesManifest { version: env!("CARGO_PKG_VERSION").into(), packages: HashMap::default() }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Package {
    pub author: String,
    /// Last github query
    pub updated: u64,
    /// GitHub tag
    pub version: String,
}

fn parse_manifest(manifest_file: &mut File) -> Result<PackagesManifest> {
    let mut contents = String::new();
    manifest_file.read_to_string(&mut contents)?;
    toml::from_str(&contents).with_context(|| {
        format!(
            "Failed to parse the system packages manifest at {}",
            style(manifest_file.path().display()).bold()
        )
    })
}

/// Clears the current console line
/// Returns the current manifest (created if nonexistent) and the file with a lock
pub fn get_system_packages_manifest(
    manifest_path: &Path,
    progress_bar: &ProgressBar,
) -> Result<(PackagesManifest, File)> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(manifest_path)?;
    progress_bar.set_message("Obtaining lock on manifest...");
    file.lock()?;
    Ok((parse_manifest(&mut file)?, file))
}

pub fn read_system_packages_manifest(manifest_path: &Path) -> Result<PackagesManifest> {
    let Ok(mut file) = File::open(manifest_path) else {
        return Ok(PackagesManifest::default());
    };
    file.lock_shared()?;
    parse_manifest(&mut file)
}

#[derive(PartialEq)]
pub enum PkgInstallStatus {
    AlreadyInstalled,
    DifferentRepoExists,
    NotInstalled,
}

pub fn add_package_to_manifest(
    system_pkg_manifest: &mut PackagesManifest,
    author: &str,
    repo_name: &str,
    tag: &str,
) {
    system_pkg_manifest.packages.insert(
        repo_name.to_string(),
        Package {
            author: author.to_string(),
            updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            version: tag.to_string(),
        },
    );
}

pub fn write_new_manifest(manifest: &PackagesManifest, manifest_file: &mut File) -> Result<()> {
    let contents =
        toml::to_string(manifest).context("Failed to encode the system packages manifest")?;
    manifest_file.set_len(0)?;
    manifest_file.rewind()?;
    manifest_file.write_all(contents.as_bytes())?;
    Ok(())
}

pub fn is_package_already_installed(
    system_pkg_manifest: &PackagesManifest,
    author: &str,
    repo_name: &str,
    tag: &str,
) -> PkgInstallStatus {
    if let Some(pkg) = system_pkg_manifest.packages.get(repo_name) {
        if pkg.author == author {
            if pkg.version == tag {
                PkgInstallStatus::AlreadyInstalled
            } else {
                PkgInstallStatus::NotInstalled
            }
        } else {
            PkgInstallStatus::DifferentRepoExists
        }
    } else {
        PkgInstallStatus::NotInstalled
    }
}
