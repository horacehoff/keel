use owo_colors::OwoColorize;
use owo_colors::colors::css::Gray;
use std::io::Write;
use std::path::Path;

use crate::{CliError, packages::read_system_packages_manifest};

pub fn list_installed_packages(
    keel_home_libs_packages_toml: &Path,
    keel_home_libs: &Path,
) -> Result<(), CliError> {
    let system_pkg_manifest = read_system_packages_manifest(keel_home_libs_packages_toml)?;
    let handle = std::io::stdout().lock();
    let mut bufwriter = std::io::BufWriter::new(handle);
    const PRINT_ERR_MSG: &str = "Unable to print. This is very weird..";

    if system_pkg_manifest.packages.is_empty() {
        writeln!(
            bufwriter,
            "No installed global packages {}{}{}",
            "(".italic().fg::<Gray>(),
            keel_home_libs.display().italic().fg::<Gray>(),
            ")".italic().fg::<Gray>(),
        )
        .expect(PRINT_ERR_MSG);
    } else {
        writeln!(
            bufwriter,
            "Installed global packages {}{}{}",
            "(".italic().fg::<Gray>(),
            keel_home_libs.display().italic().fg::<Gray>(),
            ")".italic().fg::<Gray>(),
        )
        .expect(PRINT_ERR_MSG);

        for (pkg_name, pkg) in system_pkg_manifest.packages {
            writeln!(
                bufwriter,
                " - {pkg_name} {} {}{}{}{}{}",
                pkg.version,
                '('.italic().fg::<Gray>(),
                pkg.author.italic().fg::<Gray>(),
                '/'.italic().fg::<Gray>(),
                pkg_name.italic().fg::<Gray>(),
                ')'.italic().fg::<Gray>(),
            )
            .expect(PRINT_ERR_MSG);
        }
    }

    bufwriter.flush().expect(PRINT_ERR_MSG);
    Ok(())
}
