use owo_colors::OwoColorize;
use std::path::Path;

use log::info;

use crate::{
    CliError,
    install::parse_repository,
    packages::{get_system_packages_manifest, write_new_manifest},
};

pub fn uninstall_library(
    repository: &mut String,
    keel_home_libs: &Path,
    keel_home_libs_packages_toml: &Path,
) -> Result<(), CliError> {
    let (author, repo_name, tag) = parse_repository(repository, false)?;
    let (mut system_pkg_manifest, mut manifest_file) =
        get_system_packages_manifest(keel_home_libs_packages_toml)?;
    if let Some(pkg) = system_pkg_manifest.packages.get(repo_name) {
        if !author.is_empty() && pkg.author != author {
            info!(
                "{repo_name} is installed but the package is from a different author: {}.",
                pkg.author.bold()
            );
            return Ok(());
        }
        if tag.is_some_and(|version| pkg.version != version) {
            info!("{repo_name}@{} is not installed.", tag.unwrap());
            info!("{repo_name}@{} is, however, installed.", pkg.version);
            return Ok(());
        }
    } else {
        info!("{repo_name} is not installed.");
        return Ok(());
    }

    let _ = system_pkg_manifest.packages.remove(repo_name);
    std::fs::remove_dir_all(keel_home_libs.join(repo_name)).map_err(|_| {
        CliError::FailedToRemovePkgDir {
            path: keel_home_libs.join(repo_name).display().to_string(),
        }
    })?;
    write_new_manifest(&system_pkg_manifest, &mut manifest_file, keel_home_libs_packages_toml)?;
    println!("{} Uninstalled {repo_name}!", "✓".bright_green().bold());
    Ok(())
}
