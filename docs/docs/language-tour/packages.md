---
icon: lucide/package
---
# Packages

Packages are Keel libraries published as Github releases.
They are managed with `keel install`, `keel uninstall`, and `keel list` and they're installed globally in `~/.keel/libs/` (in an upcoming release you will be able to install them locally in projects).

## Installing packages

`keel install author/repo` installs the latest release. `keel install author/repo@tag` installs the release with this specific GitHub tag.

Packages are installed in `~/.keel/libs/<repo>/` and can then be imported with `import "<repo>";`.
Please note that only one version of each package name can be installed at a time (attempting to install another version replaces the current one).

## Uninstalling packages

`keel uninstall repo`, or `repo@tag` or `author/repo`, or `author/repo@tag`.

## Publishing a package

For an example, view [the path-utils repository](https://github.com/horacehoff/path-utils).

- The release tag must be a semantic version (a leading 'v' is accepted).
- The release asset must be named `<repo><suffix>.tar.gz`. The suffixes tried are the following in order: `-<os>-<arch>`, `-<os>`, `-<arch>`, then none. OS is `macos`, `linux`, or `windows`. Arch is `aarch64` or `x86_64`.
- The archive must contain `lib.kl` at its root or inside a single top-level folder.
- Platform-specific assets are especially useful for packages that bundle [dynamic libraries](dynamic-libraries.md).