use anyhow::Result;
use console::style;
use indicatif::ProgressBar;

use crate::{
    install::parse_repository,
    packages::{get_system_packages_manifest, write_new_manifest},
};
use std::path::Path;

pub fn uninstall_library(
    mut repository: String,
    keel_home_libs: &Path,
    manifest_path: &Path,
) -> Result<()> {
    let (author, repo_name, tag) = parse_repository(&mut repository, false)?;
    let progress_bar = ProgressBar::new_spinner();
    let (mut manifest, mut manifest_file) =
        get_system_packages_manifest(manifest_path, &progress_bar)?;
    progress_bar.finish_and_clear();

    let Some(pkg) = manifest.packages.get(repo_name) else {
        println!("{repo_name} is not installed.");
        return Ok(());
    };
    if !author.is_empty() && pkg.author != author {
        println!(
            "{repo_name} is installed but the package is from a different author: {}.",
            style(&pkg.author).bold()
        );
        return Ok(());
    }
    if let Some(version) = tag
        && pkg.version != version
    {
        println!(
            "{repo_name}@{version} is not installed. {repo_name}@{} is, however, installed.",
            pkg.version
        );
        return Ok(());
    }

    manifest.packages.remove(repo_name);
    if let Err(e) = fs_err::remove_dir_all(keel_home_libs.join(repo_name))
        && e.kind() != std::io::ErrorKind::NotFound
    {
        return Err(e.into());
    }
    write_new_manifest(&manifest, &mut manifest_file)?;
    println!("{} Uninstalled {repo_name}!", style("✓").green().bright().bold());
    Ok(())
}
