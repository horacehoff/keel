use crate::packages::read_system_packages_manifest;
use crate::secondary;
use anyhow::Result;
use std::io::Write;
use std::path::Path;

pub fn list_installed_packages(manifest_path: &Path, keel_home_libs: &Path) -> Result<()> {
    let manifest = read_system_packages_manifest(manifest_path)?;
    let mut buf = std::io::BufWriter::new(std::io::stdout().lock());

    writeln!(
        buf,
        "{} {}",
        if manifest.packages.is_empty() {
            "No installed global packages"
        } else {
            "Installed global packages"
        },
        secondary(format!("({})", keel_home_libs.display()))
    )?;
    for (pkg_name, pkg) in &manifest.packages {
        writeln!(
            buf,
            " - {pkg_name} {} {}",
            pkg.version,
            secondary(format!("({}/{pkg_name})", pkg.author))
        )?;
    }

    buf.flush()?;
    Ok(())
}
