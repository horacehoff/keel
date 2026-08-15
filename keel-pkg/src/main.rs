use clap::{Parser, Subcommand};
use constcat::concat;
use futures_util::stream::StreamExt;
use indicatif::{HumanBytes, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use owo_colors::colors::css::Gray;
use reqwest::Client;
use reqwest::header::ACCEPT;
use std::cmp::min;
use std::fs::File;
use std::io::Write;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Install { repository: String },
}

pub async fn download_lib_from_github(
    client: &Client,
    name: &str,
    download_url: &str,
    file_path: &str,
) {
    let github_response =
        client.get(download_url).send().await.expect(&format!("Failed to GET {}", download_url));
    let content_length = github_response.content_length();

    let progress_bar = if let Some(size) = content_length {
        ProgressBar::new(size)
    } else {
        ProgressBar::new_spinner()
    };
    progress_bar.set_style(ProgressStyle::default_bar()
        .template("{msg}\n[{elapsed_precise}] [{wide_bar:.blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap());
    progress_bar.set_message(format!(
        "[1/2] Downloading {} from {}",
        name.blue().bold(),
        download_url.italic().fg::<Gray>()
    ));

    let mut temp_file =
        File::create(file_path).expect(&format!("Failed to create temporary file {}", file_path));
    let mut downloaded: u64 = 0;
    let mut bytes_stream = github_response.bytes_stream();

    while let Some(b) = bytes_stream.next().await {
        // tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let chunk = b.expect("Could not get the chunk");
        temp_file.write_all(&chunk).expect("Could not write to file");
        let new_pos = if let Some(size) = content_length {
            min(downloaded + (chunk.len() as u64), size)
        } else {
            downloaded + chunk.len() as u64
        };
        downloaded = new_pos;
        progress_bar.set_position(new_pos);
    }

    progress_bar.finish_and_clear();
    println!(
        "[1/2] Downloaded {} from {} ({})",
        name.blue().bold(),
        download_url.italic().fg::<Gray>(),
        HumanBytes(downloaded)
    );
}

#[derive(serde::Deserialize, Debug)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize, Debug)]
struct GithubRelease {
    assets: Vec<GithubReleaseAsset>,
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

const ARCHIVE_EXTENSIONS: [&str; 2] = [".tar.gz", ".zip"];

const SUFFIXES: [&str; 3] = [concat!(OS_SUFFIX, ARCH_SUFFIX), OS_SUFFIX, ARCH_SUFFIX];

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

fn get_github_release_asset(github_release: GithubRelease, repo_name: &str) -> GithubReleaseAsset {
    github_release
        .assets
        .into_iter()
        .find(|asset| {
            for suffix in SUFFIXES {
                for extension in ARCHIVE_EXTENSIONS {
                    if asset.name == format!("{repo_name}{suffix}{extension}") {
                        return true;
                    }
                }
            }
            false
        })
        .expect("Couldn't find a valid asset")
}

#[tokio::main]
async fn main() {
    // DON'T USE THIS YET!!!!!
    // IT'S VERY WIP!!
    let keel_home_libs = std::env::home_dir()
        .map(|p| p.join(".keel").join("libs/"))
        .expect("Can't find your home directory!");
    std::fs::create_dir_all(keel_home_libs).expect("Unable to check the existence of ~/.keel/libs");

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
            let github_asset = get_github_release_asset(github_release, repo_name);
            print!("\r[1/2] Found a suitable release asset: {}\n", github_asset.name);
            std::io::stdout().flush().unwrap();
            download_lib_from_github(
                &client,
                &repository,
                &github_asset.browser_download_url,
                &github_asset.name,
            )
            .await;
        }
        None => {}
    }
}
