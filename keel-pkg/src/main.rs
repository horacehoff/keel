use clap::{Parser, Subcommand};
use constcat::concat;
use flate2::read::GzDecoder;
use futures_util::TryStreamExt;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use owo_colors::colors::css::Gray;
use reqwest::Client;
use reqwest::header::ACCEPT;
use std::io::Write;
use std::path::PathBuf;
use tar::Archive;
use thiserror::Error;
use tokio_util::io::{StreamReader, SyncIoBridge};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install { repository: String },
}

const TAB: &str = "  ";

#[derive(Error, Debug)]
enum CliError {
    #[error("{TAB}Connection refused.\n{TAB}URL: {}.\n{TAB}Make sure you're connected to the internet.",url.italic().fg::<Gray>())]
    ConnectionRefused { url: String },
    #[error("{TAB}Connection timeout when connecting to {}.\n{TAB}Make sure you're connected to the internet.",url.italic().fg::<Gray>())]
    ConnectionTimeout { url: String },
    #[error("{TAB}Invalid URL: {}.",url.italic().fg::<Gray>())]
    InvalidURL { url: String },
    #[error("{TAB}Request failed to {}.\n{TAB}Make sure you're connected to the internet.",url.italic().fg::<Gray>())]
    RequestFailed {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{TAB}Failed to create a temporary folder at {}.\n{TAB}Check permissions.", path.display())]
    CannotCreateTemporaryFolder { path: PathBuf },
    #[error(
        "{TAB}Internal error.\n{TAB}Please file a bug report at https://github.com/horacehoff/keel/issues.\n{TAB}Details: {details}."
    )]
    InternalBug { details: String },
    #[error("{TAB}The download was truncated.\n{TAB}Only {} were downloaded out of {} ({}%).\n{TAB}Please try again.",HumanBytes(*downloaded), HumanBytes(*total), downloaded * 100 / total)]
    DownloadTruncated { downloaded: u64, total: u64 },
    #[error("{TAB}The extraction failed when downloading {}.\n{TAB}Please try again.", url.italic().fg::<Gray>())]
    TarGzExtractionFailed {
        url: String,
        #[source]
        source: std::io::Error,
    },
}

async fn download_lib_from_github(
    client: &Client,
    name: &str,
    download_url: &str,
    file_path: &str,
    repo_name: &str,
    repo_tag: &str,
    keel_home_libs: PathBuf,
) -> Result<(), CliError> {
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
    progress_bar.set_style(unsafe {ProgressStyle::default_bar()
        .template("{msg}\n[{elapsed_precise}] [{wide_bar:.blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap_unchecked()});
    progress_bar.set_message(format!(
        "[2/2] Downloading and extracting {} from {}",
        name.blue().bold(),
        download_url.italic().fg::<Gray>()
    ));

    let temp_folder = std::path::PathBuf::from(format!("{repo_name}@{repo_tag}"));
    std::fs::create_dir_all(&temp_folder)
        .map_err(|_| CliError::CannotCreateTemporaryFolder { path: temp_folder.clone() })?;
    let bytes_stream = github_response.bytes_stream().map_err(std::io::Error::other);
    let stream_reader = StreamReader::new(bytes_stream);

    let bytes_reader = SyncIoBridge::new(progress_bar.wrap_async_read(stream_reader));

    let temp_folder_unpack_dest = temp_folder.clone();
    let extraction_process = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let gzip_decompressor = GzDecoder::new(bytes_reader);
        Archive::new(gzip_decompressor).unpack(&temp_folder_unpack_dest)
    })
    .await
    .map_err(|e| CliError::InternalBug { details: e.to_string() })?;

    let downloaded = progress_bar.position();

    if let Err(e) = extraction_process {
        #[allow(unused_must_use)]
        std::fs::remove_dir_all(temp_folder);
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
    println!(
        "[2/2] Downloaded {} from {} ({})",
        name.blue().bold(),
        download_url.italic().fg::<Gray>(),
        HumanBytes(downloaded)
    );
    Ok(())
}

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

#[cfg(target_os = "macos")]
const OS_SUFFIX: &str = "-macos";
#[cfg(target_os = "linux")]
const OS_SUFFIX: &str = "-linux";
#[cfg(target_os = "windows")]
const OS_SUFFIX: &str = "-windows";

#[cfg(target_arch = "aarch64")]
const ARCH_SUFFIX: &str = "-aarch64";
#[cfg(target_arch = "x86_64")]
const ARCH_SUFFIX: &str = "-x86_64";
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const ARCH_SUFFIX: &str = "";

const SUFFIXES: [&str; 4] = [concat!(OS_SUFFIX, ARCH_SUFFIX), OS_SUFFIX, ARCH_SUFFIX, ""];

fn parse_repository(repo: &str) -> (&str, &str, Option<&str>) {
    let (author_and_repo, tag) = match repo.split_once('@') {
        Some((r, t)) => (r, Some(t)),
        None => (repo, None),
    };
    let (author, repo_name) =
        author_and_repo.split_once('/').expect("Expected author/repo[@tag], got idk");
    (author, repo_name, tag)
}

async fn get_github_release(url: &str, client: &Client) -> GithubRelease {
    let request = client.get(url).header(ACCEPT, "application/vnd.github.v3+json");
    let result = request.send().await.expect("Failed to access GitHub's API");
    let release: GithubRelease = result.json().await.expect("Filed");
    release
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

#[tokio::main]
async fn main() {
    if let Err(e) = cli().await {
        eprintln!("{}\n{}", "KEEL-PKG Error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn cli() -> Result<(), CliError> {
    // DON'T USE THIS YET!!!!!
    // IT'S VERY WIP!!
    let keel_home_libs = std::env::home_dir()
        .map(|p| p.join(".keel").join("libs/"))
        .expect("Can't find your home directory!");
    std::fs::create_dir_all(&keel_home_libs)
        .expect("Unable to check the existence of ~/.keel/libs");

    let user_agent =
        Client::builder().user_agent(concat!("keel-pkg@{}", env!("CARGO_PKG_VERSION")));

    let args = Cli::parse();
    match args.command {
        Some(Commands::Install { repository }) => {
            let client = user_agent.build().unwrap();
            let (author, repo_name, tag) = parse_repository(&repository);
            let github_api_url = if let Some(release_tag) = tag {
                format!(
                    "https://api.github.com/repos/{author}/{repo_name}/releases/tags/{release_tag}"
                )
            } else {
                format!("https://api.github.com/repos/{author}/{repo_name}/releases/latest")
            };
            print!("[1/2] Querying the GitHub API...");
            std::io::stdout().flush().unwrap();
            let github_release = get_github_release(&github_api_url, &client).await;
            let github_asset = get_github_release_asset(&github_release, repo_name);
            print!(
                "\r[1/2] Found a suitable release asset: {}\n",
                github_asset.name.italic().fg::<Gray>()
            );
            std::io::stdout().flush().unwrap();
            // let github_asset =
            //     GithubReleaseAsset { name: "test".into(), browser_download_url: "".into() };
            // let repo_name = "path-utils";
            // let github_release =
            //     GithubRelease { assets: Vec::new(), tag_name: String::from("v0.0.1") };
            download_lib_from_github(
                &client,
                &repository,
                &github_asset.browser_download_url,
                &github_asset.name,
                repo_name,
                &github_release.tag_name,
                keel_home_libs,
            )
            .await?;
        }
        None => {}
    }
    Ok(())
}
