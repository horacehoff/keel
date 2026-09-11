use clap::{Parser, Subcommand};
use console_utils::control::clear_line;
use constcat::concat;
use fern::colors::{Color, ColoredLevelConfig};
use indicatif::HumanBytes;
use log::error;
use owo_colors::OwoColorize;
use owo_colors::colors::css::Gray;
use reqwest::{Client, StatusCode};
use std::hint::cold_path;
use thiserror::Error;

use crate::install::install_library;
use crate::list::list_installed_packages;
use crate::uninstall::uninstall_library;

mod install;
mod list;
mod packages;
mod uninstall;

#[derive(Parser)]
#[command(about, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package system-wide
    Install {
        /// The GitHub repository of the package to install. Format: `author/repository_name[@tag]`
        repository: String,
        /// Force the installation of the package, potentially overwriting a previously-installed package
        #[arg(short, long)]
        force: bool,
    },
    /// Uninstall a system-wide package
    Uninstall {
        /// The Github repository of the package to uninstall. Format: `[author/]repository_name[@tag]`
        repository: String,
    },
    /// List all installed global packages
    List,
}

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Connection refused.\nURL: {}.\nMake sure you're connected to the internet.",url.italic().fg::<Gray>())]
    ConnectionRefused { url: String },
    #[error("Connection timeout when connecting to {}.\nMake sure you're connected to the internet.",url.italic().fg::<Gray>())]
    ConnectionTimeout { url: String },
    #[error("Invalid URL: {}.",url.italic().fg::<Gray>())]
    InvalidURL { url: String },
    #[error("Request failed to {}.\nMake sure you're connected to the internet.",url.italic().fg::<Gray>())]
    RequestFailed {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to create temporary folder: {}.\nCheck permissions.", path.bold())]
    CannotCreateFolder { path: String },
    #[error(
        "Internal error.\nPlease file a bug report at https://github.com/horacehoff/keel/issues.\nDetails: {details}."
    )]
    InternalBug { details: String },
    #[error("The download was truncated.\nOnly {} were downloaded out of {} ({}%).\nPlease try again.",HumanBytes(*downloaded), HumanBytes(*total), downloaded * 100 / total)]
    DownloadTruncated { downloaded: u64, total: u64 },
    #[error("The extraction failed when downloading {}.\nPlease try again.", url.italic().fg::<Gray>())]
    TarGzExtractionFailed {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Could not find tag {} in repository {} {}{}{}{}.", tag.bold(), repository.bold(), "(".italic().fg::<Gray>(),"https://github.com/".italic().fg::<Gray>(),repository.italic().fg::<Gray>(),")".italic().fg::<Gray>())]
    UnknownTagOrRepo { tag: String, repository: String },
    #[error(
        "GitHub's rate limit has been reached. Try again in a few minutes, this should be resolved in less than an hour."
    )]
    GithubTooManyRequests,
    #[error("Request to GitHub failed.\nStatus code: {}\nMessage: {}", status_code.bold(), message.bold())]
    GithubError { status_code: StatusCode, message: String },
    #[error("{} is not a valid repository.", repository.bold())]
    InvalidRepo { repository: String },
    #[error("{} is not a valid tag or version.", tag.bold())]
    InvalidTagOrVersion { tag: String },
    #[error("There is already a package named {} installed.\nTo override this and uninstall it, use the -f/--force flag.", repo_name.bold())]
    PkgWithNameAlreadyExists { repo_name: String },
    #[error("Cannot read path {}",path.bold())]
    CannotReadPath { path: String },
    #[error("The downloaded folder is empty!")]
    DownloadedFolderIsEmpty,
    #[error("Failed to create/open the system manifest at {}", path.bold())]
    CannotOpenPkgManifest { path: String },
    #[error("Filed to acquire a lock on the system manifest at {}", path.bold())]
    CannotAcquireLockOnPkgManifest { path: String },
    #[error("Failed to read the system manifest at {}", path.bold())]
    CannotReadPkgManifest { path: String },
    #[error("Failed to write to the system manifest at {}", path.bold())]
    CannotWritePkgManifest { path: String },
    #[error("Failed to TOML-parse the system manifest at {}", path.bold())]
    FailedToParsePkgManifest { path: String },
    #[error("Failed to find a valid release asset in `{}` that fit the following:\n - {repo_name}{}.tar.gz\n - {repo_name}{}.tar.gz\n - {repo_name}{}.tar.gz\n - {repo_name}{}.tar.gz", repo_name.bold(), SUFFIXES[0],SUFFIXES[1],SUFFIXES[2],SUFFIXES[3], )]
    FailedToFindValidReleaseAsset { repo_name: String },
    #[error("Failed to remove package directory {}", path.bold())]
    FailedToRemovePkgDir { path: String },
    #[error("{} is not a valid package name.", repo_name.bold())]
    InvalidPkgName { repo_name: String },
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

pub const SUFFIXES: [&str; 4] = [concat!(OS_SUFFIX, ARCH_SUFFIX), OS_SUFFIX, ARCH_SUFFIX, ""];

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = cli().await {
        cold_path();
        clear_line();
        error!("{e}");
        std::process::exit(1);
    }
}

async fn cli() -> Result<(), CliError> {
    let colors = ColoredLevelConfig::new().error(Color::Red).warn(Color::Yellow).info(Color::White);
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{level}] {message}\x1B[0m",
                level = colors.color(record.level()),
                message = message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(
            fern::Dispatch::new()
                .filter(|metadata| metadata.level() != log::LevelFilter::Error)
                .chain(std::io::stdout()),
        )
        .chain(fern::Dispatch::new().level(log::LevelFilter::Error).chain(std::io::stderr()))
        .apply()
        .unwrap();

    // WIP!
    let keel_home =
        std::env::home_dir().map(|p| p.join(".keel")).expect("Can't find your home directory!");
    let keel_home_libs = keel_home.join("libs/");
    let keel_home_libs_packages_toml = keel_home_libs.join(".packages.toml");
    std::fs::create_dir_all(&keel_home_libs)
        .expect("Unable to check the existence of ~/.keel/libs");

    let user_agent = Client::builder().user_agent(concat!("keel-pkg@", env!("CARGO_PKG_VERSION")));

    let args = Cli::parse();
    match args.command {
        Commands::Install { mut repository, force } => {
            install_library(
                &mut repository,
                user_agent,
                &keel_home,
                &keel_home_libs,
                &keel_home_libs_packages_toml,
                force,
            )
            .await?;
        }
        Commands::Uninstall { mut repository } => {
            uninstall_library(&mut repository, &keel_home_libs, &keel_home_libs_packages_toml)?
        }
        Commands::List => {
            list_installed_packages(&keel_home_libs_packages_toml, &keel_home_libs)?;
        }
    }
    Ok(())
}
