use std::io::{Seek, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, io::Read, path::Path};

use console_utils::control::clear_line;
use serde::{Deserialize, Serialize};

use crate::CliError;

#[derive(Deserialize, Serialize)]
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
/// Returns the current manifest (created if nonexistent) and the file with a lock
pub fn get_system_packages_manifest(
    keel_home_libs_packages_toml: &Path,
) -> Result<(PackagesManifest, std::fs::File), CliError> {
    let mut file = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(keel_home_libs_packages_toml)
        .map_err(|_| CliError::CannotOpenPkgManifest {
            path: keel_home_libs_packages_toml.display().to_string(),
        })?;
    clear_line();
    print!("Obtaining lock on manifest...");
    std::io::stdout().flush().unwrap();
    file.lock().map_err(|_| CliError::CannotAcquireLockOnPkgManifest {
        path: keel_home_libs_packages_toml.display().to_string(),
    })?;
    clear_line();
    let mut contents = String::with_capacity(file.metadata().unwrap().len() as usize);
    file.read_to_string(&mut contents).map_err(|_| CliError::CannotReadPkgManifest {
        path: keel_home_libs_packages_toml.display().to_string(),
    })?;

    let manifest: PackagesManifest = if contents.is_empty() {
        let manifest = PackagesManifest::default();
        file.write_all(
            toml::to_string(&manifest)
                .map_err(|_| CliError::InternalBug { details: "Default system manifest".into() })?
                .as_bytes(),
        )
        .map_err(|_| CliError::CannotWritePkgManifest {
            path: keel_home_libs_packages_toml.display().to_string(),
        })?;
        manifest
    } else {
        toml::from_str(&contents).map_err(|_| CliError::FailedToParsePkgManifest {
            path: keel_home_libs_packages_toml.display().to_string(),
        })?
    };

    Ok((manifest, file))
}

pub fn read_system_packages_manifest(
    keel_home_libs_packages_toml: &Path,
) -> Result<PackagesManifest, CliError> {
    if let Ok(mut file) = std::fs::File::options().read(true).open(keel_home_libs_packages_toml) {
        file.lock_shared().map_err(|_| CliError::CannotAcquireLockOnPkgManifest {
            path: keel_home_libs_packages_toml.display().to_string(),
        })?;
        let mut contents = String::with_capacity(file.metadata().unwrap().len() as usize);
        file.read_to_string(&mut contents).map_err(|_| CliError::CannotReadPkgManifest {
            path: keel_home_libs_packages_toml.display().to_string(),
        })?;

        let manifest: PackagesManifest = if contents.is_empty() {
            PackagesManifest::default()
        } else {
            toml::from_str(&contents).map_err(|_| CliError::FailedToParsePkgManifest {
                path: keel_home_libs_packages_toml.display().to_string(),
            })?
        };

        Ok(manifest)
    } else {
        Ok(PackagesManifest::default())
    }
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

pub fn write_new_manifest(
    manifest: &PackagesManifest,
    manifest_file: &mut std::fs::File,
    manifest_path: &Path,
) -> Result<(), CliError> {
    manifest_file.set_len(0).map_err(|_| CliError::CannotWritePkgManifest {
        path: manifest_path.display().to_string(),
    })?;
    manifest_file.seek(std::io::SeekFrom::Start(0)).map_err(|_| {
        CliError::CannotWritePkgManifest { path: manifest_path.display().to_string() }
    })?;
    manifest_file
        .write_all(
            toml::to_string(manifest)
                .map_err(|_| CliError::CannotWritePkgManifest {
                    path: manifest_path.display().to_string(),
                })?
                .as_bytes(),
        )
        .map_err(|_| CliError::CannotWritePkgManifest {
            path: manifest_path.display().to_string(),
        })?;
    manifest_file.flush().map_err(|_| CliError::CannotWritePkgManifest {
        path: manifest_path.display().to_string(),
    })?;
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
