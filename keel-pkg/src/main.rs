use anyhow::Context;
use anyhow::Result;
use clap::{Parser, Subcommand};
use console::style;
use constcat::concat;
use reqwest::Client;
use std::hint::cold_path;

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

pub const BUG_NOTICE: &str =
    "Internal error. Please report this at https://github.com/horacehoff/keel/issues.\nDetails: ";

pub fn secondary<F>(value: F) -> console::StyledObject<F> {
    style(value).italic().true_color(128, 128, 128)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = cli().await {
        cold_path();
        eprintln!("[{}] {e:#}", style("ERROR").red().for_stderr());
        std::process::exit(1);
    }
}

async fn cli() -> Result<()> {
    let keel_home_libs =
        std::env::home_dir().context("Cannot find the home directory!")?.join(".keel/libs/");
    let manifest_path = keel_home_libs.join(".packages.toml");
    std::fs::create_dir_all(&keel_home_libs)
        .expect("Unable to check the existence of ~/.keel/libs");

    let user_agent = Client::builder().user_agent(concat!("keel-pkg@", env!("CARGO_PKG_VERSION")));

    let args = Cli::parse();
    match args.command {
        Commands::Install { repository, force } => {
            install_library(repository, user_agent, &keel_home_libs, &manifest_path, force).await
        }
        Commands::Uninstall { repository } => {
            uninstall_library(repository, &keel_home_libs, &manifest_path)
        }
        Commands::List => list_installed_packages(&manifest_path, &keel_home_libs),
    }
}
