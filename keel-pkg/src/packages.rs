use std::io::Write;
use std::{collections::HashMap, io::Read, path::Path};

use console_utils::control::clear_line;
use serde::{Deserialize, Serialize};

use crate::CliError;

#[derive(Deserialize, Default, Serialize)]
pub struct PackagesManifest {
    pub packages: HashMap<String, Package>,
}

#[derive(Deserialize, Serialize)]
pub struct Package {
    pub repository: String,
    /// Last github query
    pub updated: toml::value::Datetime,
    pub latest: String,
    /// each string is a tag
    pub installed: Vec<String>,
}

/// Clears the current console line
/// Returns the current manifest (created if nonexistent) and the file with a lock
pub fn read_system_packages_manifest(
    keel_home_libs_packages_toml: &Path,
) -> Result<(PackagesManifest, std::fs::File), CliError> {
    let mut file = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(keel_home_libs_packages_toml)
        .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
    clear_line();
    print!("Obtaining lock on manifest...");
    std::io::stdout().flush().unwrap();
    file.lock().map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
    clear_line();
    let mut contents = String::with_capacity(file.metadata().unwrap().len() as usize);
    file.read_to_string(&mut contents)
        .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;

    let manifest: PackagesManifest = if contents.is_empty() {
        let manifest = PackagesManifest::default();
        file.write_all(
            toml::to_string(&manifest)
                .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?
                .as_bytes(),
        )
        .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
        manifest
    } else {
        toml::from_str(&contents).map_err(|_| CliError::CannotCreateFolder { path: "".into() })?
    };

    Ok((manifest, file))
}
