use crate::packages::read_system_packages_manifest;
use crate::{CliError, SUFFIXES};
use console_utils::control::clear_line;
use flate2::read::GzDecoder;
use futures_util::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use owo_colors::colors::css::Gray;
use reqwest::header::ACCEPT;
use reqwest::{Client, ClientBuilder};
use serde::Deserialize;
use std::hint::cold_path;
use std::io::Write;
use std::path::Path;
use tar::Archive;
use tokio_util::io::{StreamReader, SyncIoBridge};

#[derive(serde::Deserialize, Debug)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize, Debug)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
    tag_name: String,
}

fn parse_repository(repo: &str) -> (&str, &str, Option<&str>) {
    let (author_and_repo, tag) = match repo.split_once('@') {
        Some((r, t)) => (r, Some(t)),
        None => (repo, None),
    };
    let (author, repo_name) =
        author_and_repo.split_once('/').expect("Expected author/repo[@tag], got idk");
    (author, repo_name, tag)
}

#[derive(Deserialize)]
struct GitHubError {
    message: String,
}

async fn get_github_release(
    url: &str,
    author: &str,
    repo_name: &str,
    tag: &str,
    client: &Client,
) -> Result<GithubRelease, CliError> {
    let request = client.get(url).header(ACCEPT, "application/vnd.github.v3+json");
    let result = request.send().await.expect("Failed to access GitHub's API");
    let status_code = result.status();
    if status_code.is_success() {
        result
            .json::<GithubRelease>()
            .await
            .map_err(|_| CliError::InternalBug { details: "Cannot parse release".into() })
    } else {
        cold_path();
        let error_response =
            result.text().await.map_err(|_| CliError::InternalBug { details: "".into() })?;
        let error_json = serde_json::from_str::<GitHubError>(&error_response)
            .map_err(|_| CliError::GithubError { status_code, message: error_response })?;
        Err(CliError::GithubError { status_code, message: error_json.message })
    }
}

fn get_github_release_asset<'a>(
    github_release: &'a GithubRelease,
    repo_name: &str,
) -> &'a GithubReleaseAsset {
    github_release
        .assets
        .iter()
        .find(|asset| {
            for suffix in SUFFIXES {
                if asset.name == format!("{repo_name}{suffix}.tar.gz") {
                    return true;
                }
            }
            false
        })
        .expect("Couldn't find a valid asset")
}

pub async fn install_library(
    repository: &str,
    user_agent: ClientBuilder,
    keel_home: &Path,
    keel_home_libs: &Path,
    keel_home_libs_packages_toml: &Path,
) -> Result<(), CliError> {
    let client = user_agent.build().unwrap();
    let (author, repo_name, tag) = parse_repository(repository);
    let github_api_url = if let Some(release_tag) = tag {
        format!("https://api.github.com/repos/{author}/{repo_name}/releases/tags/{release_tag}")
    } else {
        format!("https://api.github.com/repos/{author}/{repo_name}/releases/latest")
    };
    print!("Querying the GitHub API...");
    std::io::stdout().flush().unwrap();
    let github_release =
        get_github_release(&github_api_url, author, repo_name, tag.unwrap_or("latest"), &client)
            .await?;
    let github_asset = get_github_release_asset(&github_release, repo_name);
    let lib_folder_name = format!("{repo_name}@{}", github_release.tag_name);
    clear_line();
    print!("Found a suitable release asset: {}", github_asset.name.italic().fg::<Gray>());
    std::io::stdout().flush().unwrap();

    let (manifest_contents, manifest_file) =
        read_system_packages_manifest(keel_home_libs_packages_toml)?;

    // let download_url = "http://127.0.0.1:8000/samplearchive.tar.gz";
    let download_url = &github_asset.browser_download_url;

    let github_response = client.get(download_url).send().await.map_err(|e| {
        if e.is_connect() {
            CliError::ConnectionRefused { url: download_url.to_string() }
        } else if e.is_timeout() {
            CliError::ConnectionTimeout { url: download_url.to_string() }
        } else if e.is_builder() {
            CliError::InvalidURL { url: download_url.to_string() }
        } else {
            CliError::RequestFailed { url: download_url.to_string(), source: e }
        }
    })?;
    let content_length = github_response.content_length();

    let progress_bar = if let Some(size) = content_length {
        ProgressBar::new(size)
    } else {
        ProgressBar::new_spinner()
    };
    progress_bar.set_style(
        unsafe {
            ProgressStyle::default_bar()
                .template("{msg}\n[{bar:42}] {percent}% ({decimal_bytes}/{total_bytes})")
                .unwrap_unchecked()
        }
        .progress_chars("█▉▊▋▌▍▎▏ "),
    );
    progress_bar.set_message(format!("Downloading {}", lib_folder_name.bold()));

    let temp_lib_folder = keel_home.join("tmp/").join(&lib_folder_name);
    std::fs::create_dir_all(&temp_lib_folder)
        .map_err(|_| CliError::CannotCreateFolder { path: temp_lib_folder.clone() })?;
    let bytes_stream = github_response.bytes_stream().map_err(std::io::Error::other);
    let stream_reader = StreamReader::new(bytes_stream);

    let bytes_reader = SyncIoBridge::new(progress_bar.wrap_async_read(stream_reader));

    let temp_folder_unpack_dest = temp_lib_folder.clone();
    let extraction_process = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let gzip_decompressor = GzDecoder::new(bytes_reader);
        Archive::new(gzip_decompressor).unpack(&temp_folder_unpack_dest)
    })
    .await
    .map_err(|e| CliError::InternalBug { details: e.to_string() })?;

    let downloaded = progress_bar.position();

    if let Err(e) = extraction_process {
        let _ = std::fs::remove_dir_all(temp_lib_folder);
        return Err(
            if let Some(len) = content_length
                && downloaded < len
            {
                CliError::DownloadTruncated { downloaded, total: len }
            } else {
                CliError::TarGzExtractionFailed { url: download_url.to_string(), source: e }
            },
        );
    }
    progress_bar.finish_and_clear();
    print!("Installing {}", lib_folder_name.bold());
    std::io::stdout().flush().unwrap();

    let lib_folder = keel_home_libs.join(&lib_folder_name);

    let mut lib_entries = std::fs::read_dir(&temp_lib_folder)
        .map_err(|_| CliError::CannotCreateFolder { path: "./".into() })?;

    let first_lib_entry = lib_entries
        .next()
        .ok_or(CliError::CannotCreateFolder { path: "".into() })?
        .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;

    let _ = std::fs::remove_dir_all(&lib_folder);
    // If there's a single entry in the lib's archive (a top-levl folder or a single file), move that instead of the parent folder
    if lib_entries.next().is_none() {
        let src = first_lib_entry.path();
        let dest = if first_lib_entry.file_type().unwrap().is_dir() {
            lib_folder.clone()
        } else {
            std::fs::create_dir_all(&lib_folder)
                .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
            lib_folder.join(first_lib_entry.file_name())
        };
        std::fs::rename(src, dest).map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
    } else {
        std::fs::rename(&temp_lib_folder, &lib_folder)
            .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
    }

    let _ = std::fs::remove_dir_all(temp_lib_folder);

    let mut symlink_to_latest_created = true;
    let folder_symlink_path = keel_home_libs.join(repo_name);

    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(&folder_symlink_path);
        std::os::unix::fs::symlink(&lib_folder, &folder_symlink_path)
            .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
    }
    #[cfg(windows)]
    {
        let _ = std::fs::remove_dir(&folder_symlink_path);
        junction::create(&lib_folder, &folder_symlink_path)
            .map_err(|_| CliError::CannotCreateFolder { path: "".into() })?;
    }

    clear_line();
    println!(
        "{} Installed {} as {}! Ad maiora!",
        "✓".bright_green().bold(),
        format!("{author}/{lib_folder_name}").italic(),
        if symlink_to_latest_created { &folder_symlink_path } else { &lib_folder }
            .file_name()
            .unwrap()
            .to_string_lossy()
            .bold(),
    );

    Ok(())
}
