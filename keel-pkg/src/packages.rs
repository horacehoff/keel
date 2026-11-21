use anyhow::{Context, Result};
use console::style;
use fs_err::File;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, path::Path};

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

/// Clears the current console line
/// Returns the current manifest (default if nonexistent) and the file with a lock
pub fn get_system_packages_manifest(
    manifest_path: &Path,
    progress_bar: &ProgressBar,
) -> Result<(PackagesManifest, File)> {
    let lock = File::create(manifest_path.with_extension("lock"))?;
    progress_bar.set_message("Obtaining lock on manifest...");
    lock.lock()?;
    Ok((read_system_packages_manifest(manifest_path)?, lock))
}

pub fn read_system_packages_manifest(manifest_path: &Path) -> Result<PackagesManifest> {
    let contents = match fs_err::read_to_string(manifest_path) {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PackagesManifest::default());
        }
        Err(e) => return Err(e.into()),
    };
    toml::from_str(&contents).with_context(|| {
        format!(
            "Failed to parse the system packages manifest at {}",
            style(manifest_path.display()).bold()
        )
    })
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

pub fn write_new_manifest(manifest: &PackagesManifest, manifest_path: &Path) -> Result<()> {
    let contents =
        toml::to_string(manifest).context("Failed to encode the system packages manifest")?;
    let mut tmp = tempfile::NamedTempFile::new_in(manifest_path.parent().unwrap())?;
    tmp.write_all(contents.as_bytes())?;
    tmp.as_file().sync_all()?;
    tmp.persist(manifest_path)?;
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
