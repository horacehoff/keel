use clap::{Parser, Subcommand};
use console_utils::control::clear_line;
use constcat::concat;
use indicatif::HumanBytes;
use owo_colors::OwoColorize;
use owo_colors::colors::css::Gray;
use reqwest::{Client, StatusCode};
use std::hint::cold_path;
use std::path::PathBuf;
use thiserror::Error;

use crate::install::install_library;
use crate::list::list_installed_packages;

mod install;
mod list;
mod packages;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package system-wide
    Install {
        /// The GitHub repository of the package to install. It should be: `author/repository_name`
        repository: String,
        /// Force the installation of the package, potentially overwriting a previously-installed package
        #[arg(short, long)]
        force: bool,
    },
    /// List all installed global packages
    List,
}

pub const TAB: &str = "  ";

#[derive(Error, Debug)]
pub enum CliError {
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
    #[error("{TAB}Failed to create temporary folder: {}.\n{TAB}Check permissions.", path.display())]
    CannotCreateFolder { path: PathBuf },
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
    #[error("{TAB}Could not find tag {} in repository {} {}{}{}{}.", tag.bold(), repository.bold(), "(".italic().fg::<Gray>(),"https://github.com/".italic().fg::<Gray>(),repository.italic().fg::<Gray>(),")".italic().fg::<Gray>())]
    UnknownTagOrRepo { tag: String, repository: String },
    #[error(
        "{TAB}GitHub's rate limit has been reached. Try again in a few minutes, this should be resolved in less than an hour."
    )]
    GithubTooManyRequests,
    #[error("{TAB}Request to GitHub failed.\n{TAB}Status code: {}\n{TAB}Message: {}", status_code.bold(), message.bold())]
    GithubError { status_code: StatusCode, message: String },
    #[error("{TAB}{} is not a valid repository.", repository.bold())]
    InvalidRepo { repository: String },
    #[error("{TAB}There is already a package named {} installed.\n{TAB}To override this and uninstall it, use the -f/--force flag.", repo_name.bold())]
    PkgWithNameAlreadyExists { repo_name: String },
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
        eprintln!("{}\n{}", "KEEL-PKG Error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn cli() -> Result<(), CliError> {
    // DON'T USE THIS YET!!!!!
    // IT'S VERY WIP!!
    let keel_home =
        std::env::home_dir().map(|p| p.join(".keel")).expect("Can't find your home directory!");
    let keel_home_libs = keel_home.join("libs/");
    let keel_home_libs_packages_toml = keel_home_libs.join(".packages.toml");
    std::fs::create_dir_all(&keel_home_libs)
        .expect("Unable to check the existence of ~/.keel/libs");

    let user_agent = Client::builder().user_agent(concat!("keel-pkg@", env!("CARGO_PKG_VERSION")));

    let args = Cli::parse();
    match args.command {
        Some(Commands::Install { repository, force }) => {
            install_library(
                &repository,
                user_agent,
                &keel_home,
                &keel_home_libs,
                &keel_home_libs_packages_toml,
                force,
            )
            .await?;
        }
        Some(Commands::List) => {
            list_installed_packages(&keel_home_libs_packages_toml, &keel_home_libs)?;
        }
        None => cold_path(),
    }
    Ok(())
}
