use crate::packages::{
    PkgInstallStatus, add_package_to_manifest, get_system_packages_manifest,
    is_package_already_installed, write_new_manifest,
};
use crate::{BUG_NOTICE, SUFFIXES, TMP_PREFIX, secondary};
use anyhow::{Context, Result, anyhow, bail, ensure};
use console::style;
use flate2::read::GzDecoder;
use futures_util::TryStreamExt;
use hybrid_format::hformat;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::{ACCEPT, RETRY_AFTER};
use reqwest::{Client, ClientBuilder, StatusCode};
use semver::Version;
use serde::Deserialize;
use std::hint::cold_path;
use std::path::Path;
use std::time::Duration;
use tar::Archive;
use tokio_util::io::{StreamReader, SyncIoBridge};

#[derive(Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
    tag_name: String,
}

fn validate_repository_name(repo_name: &str) -> Result<()> {
    ensure!(
        !repo_name.is_empty()
            && !repo_name.starts_with('.')
            && repo_name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            && !repo_name.eq_ignore_ascii_case("std")
            && !repo_name.ends_with(".kl"),
        "{} is not a valid package name.",
        style(repo_name).bold()
    );
    Ok(())
}

pub fn parse_repository(
    repo: &mut String,
    author_required: bool,
) -> Result<(&str, &str, Option<&str>)> {
    let (author_and_repo, tag) = match repo.find('@') {
        Some(tag_separator) => {
            repo[..tag_separator].make_ascii_lowercase();
            (&repo[..tag_separator], Some(&repo[tag_separator + 1..]))
        }
        None => {
            repo.make_ascii_lowercase();
            (repo.as_str(), None)
        }
    };
    if let Some((author, repo_name)) = author_and_repo.split_once('/') {
        validate_repository_name(repo_name)?;
        Ok((author, repo_name, tag))
    } else if author_required {
        bail!("{} is not a valid repository", style(repo).bold());
    } else {
        validate_repository_name(author_and_repo)?;
        Ok(("", author_and_repo, tag))
    }
}

#[derive(Deserialize)]
struct GitHubError {
    message: String,
}
async fn get_github_release(
    url: &str,
    repository: &str,
    tag: &str,
    client: &Client,
) -> Result<GithubRelease> {
    let response = client
        .get(url)
        .header(ACCEPT, "application/vnd.github.v3+json")
        .send()
        .await
        .with_context(|| {
            format!(
                "Request failed to {}. Make sure you're connected to the internet.",
                secondary(url)
            )
        })?;
    let status_code = response.status();
    if status_code.is_success() {
        return response
            .json::<GithubRelease>()
            .await
            .context(hformat!("{BUG_NOTICE}Cannot parse release."));
    }
    cold_path();
    match status_code {
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            if status_code == StatusCode::TOO_MANY_REQUESTS
                || response.headers().contains_key(RETRY_AFTER)
                || response.headers().get("x-ratelimit-remaining").is_some_and(|h| h == "0") =>
        {
            bail!(
                "GitHub's rate limit has been reached. Try again in a few minutes, this should be resolved in less than an hour."
            )
        }
        StatusCode::NOT_FOUND => bail!(
            "Could not find tag {} in repository {} {}.",
            style(tag).bold(),
            style(repository).bold(),
            secondary(format!("(https://github.com/{repository})"))
        ),
        _ => {
            cold_path();
            let error_response = response.text().await.unwrap_or_default();
            let error_json = serde_json::from_str::<GitHubError>(&error_response)
                .map_or(error_response, |e| e.message);
            bail!(
                "Request to GitHub failed. Status code: {}. Message: {}.",
                style(status_code).bold(),
                style(error_json).bold()
            )
        }
    }
}

#[inline(always)]
fn get_github_release_asset<'a>(
    github_release: &'a GithubRelease,
    repo_name: &str,
) -> Result<&'a GithubReleaseAsset> {
    SUFFIXES
        .iter()
        .find_map(|suffix| {
            github_release.assets.iter().find(|asset| {
                asset.name.eq_ignore_ascii_case(&format!("{repo_name}{suffix}.tar.gz"))
            })
        })
        .with_context(|| {
            format!(
                "Failed to find a valid release asset in `{}` that fit the following: {}",
                style(repo_name).bold(),
                SUFFIXES.map(|suffix| format!("\n - {repo_name}{suffix}.tar.gz")).concat()
            )
        })
}

pub async fn install_library(
    mut repository: String,
    user_agent: ClientBuilder,
    keel_home_libs: &Path,
    manifest_path: &Path,
    force: bool,
) -> Result<()> {
    let client =
        user_agent.build().context(hformat!("{BUG_NOTICE}Cannot build the HTTP client."))?;
    let (author, repo_name, tag) = parse_repository(&mut repository, true)?;
    let github_api_url = if let Some(release_tag) = tag {
        format!("https://api.github.com/repos/{author}/{repo_name}/releases/tags/{release_tag}")
    } else {
        format!("https://api.github.com/repos/{author}/{repo_name}/releases/latest")
    };
    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(100));
    progress_bar.set_message("Querying the GitHub API...");

    let github_release = get_github_release(
        &github_api_url,
        &format!("{author}/{repo_name}"),
        tag.unwrap_or("latest"),
        &client,
    )
    .await?;
    let version = github_release.tag_name.as_str();
    Version::parse(version.strip_prefix('v').unwrap_or(version))
        .with_context(|| format!("{} is not a valid tag or version", style(version).bold()))?;
    let github_asset = get_github_release_asset(&github_release, repo_name)?;
    progress_bar
        .set_message(format!("Found a suitable release asset: {}", secondary(&github_asset.name)));

    let (mut system_pkg_manifest, _manifest_lock) =
        get_system_packages_manifest(manifest_path, &progress_bar)?;

    for f in fs_err::read_dir(keel_home_libs)?.flatten() {
        if f.file_name().to_string_lossy().starts_with(TMP_PREFIX) {
            let _ = std::fs::remove_dir_all(f.path());
        }
    }

    if !force {
        match is_package_already_installed(&system_pkg_manifest, author, repo_name, version) {
            PkgInstallStatus::AlreadyInstalled
                if keel_home_libs.join(repo_name).join("lib.kl").is_file() =>
            {
                progress_bar.finish_and_clear();
                println!(
                    "{} {} is already installed as {}! Ad maiora!",
                    style("✓").green().bright().bold(),
                    style(format!("{author}/{repo_name}")).italic(),
                    repo_name
                );
                return Ok(());
            }
            PkgInstallStatus::DifferentRepoExists => bail!(
                "There is already a package named {} installed. To override this and uninstall it, use the -f/--force flag.",
                style(repo_name).bold()
            ),
            PkgInstallStatus::NotInstalled | PkgInstallStatus::AlreadyInstalled => {
                if !system_pkg_manifest.packages.contains_key(repo_name)
                    && keel_home_libs.join(repo_name).symlink_metadata().is_ok()
                {
                    bail!(
                        "A folder named {} not managed by keel-pkg already exists. To overwrite it, use the -f/--force flag.",
                        style(repo_name).bold()
                    )
                }
            }
        }
    }

    // let download_url = "http://127.0.0.1:8000/samplearchive.tar.gz";
    let download_url = github_asset.browser_download_url.as_str();

    let github_response = client.get(download_url).send().await.and_then(|e| e.error_for_status()).map_err(|e| {
        let url = secondary(download_url);
        let message = if let Some(status) = e.status() {
            format!("Download from {url} failed with status {status}.")
        } else if e.is_connect() {
            format!("Connection refused when connecting to {url}. Make sure you're connected to the internet.")
        } else if e.is_timeout() {
            format!("Connection timeout when connecting to {url}. Make sure you're connected to the internet.")
        } else if e.is_builder() {
            format!("Invalid URL: {url}.")
        } else {
            format!("Request failed to {url}. Make sure you're connected to the internet.")
        };
        anyhow!(e).context(message)
    })?;
    let content_length = github_response.content_length();

    if let Some(size) = content_length {
        progress_bar.set_length(size);
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{msg}\n[{bar:42}] {percent}% ({decimal_bytes}/{total_bytes})",
            )
            .unwrap(),
        );
    };
    progress_bar.set_message(format!("Downloading {}", style(repo_name).bold()));

    let temp_folder = tempfile::Builder::new()
        .prefix(TMP_PREFIX)
        .tempdir_in(keel_home_libs)
        .with_context(|| {
            format!(
                "Failed to create temporary folder in {}. Check permissions.",
                style(keel_home_libs.display()).bold()
            )
        })?;
    let bytes_stream = github_response.bytes_stream().map_err(std::io::Error::other);
    let bytes_reader =
        SyncIoBridge::new(progress_bar.wrap_async_read(StreamReader::new(bytes_stream)));

    let extract_destination = temp_folder.path().to_path_buf();
    let extraction_process = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        Archive::new(GzDecoder::new(bytes_reader)).unpack(extract_destination)
    })
    .await
    .context(hformat!("{BUG_NOTICE}Archive extraction failed."))?;

    let downloaded = progress_bar.position();

    extraction_process.context(
        if let Some(len) = content_length
            && downloaded < len
        {
            "The download was truncated. Please try again."
        } else {
            "Archive extraction failed. Please try again."
        },
    )?;

    progress_bar.set_style(ProgressStyle::default_spinner());
    progress_bar.set_message(format!("Installing {}", style(repo_name).bold()));

    let lib_folder = keel_home_libs.join(repo_name);

    let mut lib_entries = fs_err::read_dir(temp_folder.path())?;

    let first_lib_entry = lib_entries.next().context("The downloaded folder is empty!")??;

    // If there's a single entry in the lib's archive (a top-levl folder or a single file), move that instead of the parent folder
    let lib_source = if lib_entries.next().is_none() && first_lib_entry.file_type()?.is_dir() {
        first_lib_entry.path()
    } else {
        temp_folder.path().to_path_buf()
    };
    ensure!(
        lib_source.join("lib.kl").is_file(),
        "The package does not contain a {} file.",
        style("lib.kl").bold()
    );

    // Backup in case the new version can'tb e moved into place
    let backup_folder = tempfile::Builder::new().prefix(TMP_PREFIX).tempdir_in(keel_home_libs)?;
    let prev_lib_path = backup_folder.path().join(repo_name);

    if lib_folder.symlink_metadata().is_ok() {
        fs_err::rename(&lib_folder, &prev_lib_path)?;
    }
    if let Err(e) = fs_err::rename(&lib_source, &lib_folder) {
        let _ = std::fs::rename(&prev_lib_path, &lib_folder);
        return Err(e.into());
    }

    add_package_to_manifest(&mut system_pkg_manifest, author, repo_name, version);

    write_new_manifest(&system_pkg_manifest, manifest_path)?;

    progress_bar.finish_and_clear();
    println!(
        "{} Installed {}@{version}! Ad maiora!\nImport it in any Keel file with `import \"{repo_name}\";`",
        style("✓").green().bright().bold(),
        style(format!("{author}/{repo_name}")).italic(),
    );

    Ok(())
}
