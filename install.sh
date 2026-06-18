#!/bin/sh

# KEEL INSTALLER

set -e

INSTALL_DIR="/usr/local/bin"

if [ -t 1 ]; then
    RED="\033[31m" GREEN="\033[32m" BOLD="\033[1m" RESET="\033[0m"
else
    RED="" GREEN="" BOLD="" RESET=""
fi

print()  { printf "${BOLD}[Keel]${RESET} %s\n" "$*"; }
error() { printf "${RED}[Keel] Error:${RESET} %s\n" "$*" >&2; exit 1; }

if command -v curl >/dev/null 2>&1; then
    # Fail silently on HTTP errors & show errors even when silent & follow redirects & show progress bar
    DOWNLOAD_CMD="curl -fSL --progress-bar"
elif command -v wget >/dev/null 2>&1; then
    # Write output to stdout & show progress bar
    DOWNLOAD_CMD="wget -O- --show-progress"
else
    error "curl or wget is required"
fi

# Supported OS's: "Darwin" on macOS, "Linux" on Linux
OS=$(uname -s)

# Supported archs: x86_64, arm64, aarch64
ARCH=$(uname -m)

case "$OS" in
    Darwin)
        case "$ARCH" in
            x86_64)  ARTIFACT="keel-x86_64-apple-darwin" ;;
            arm64)   ARTIFACT="keel-aarch64-apple-darwin" ;;
            *)       error "Unsupported macOS architecture: $ARCH" ;;
        esac
        ;;
    Linux)
        case "$ARCH" in
            x86_64)
                # Quietly check if AVX2 is supported by the current CPU on Linux
                if grep -q avx2 /proc/cpuinfo 2>/dev/null; then
                    ARTIFACT="keel-x86_64-linux-v3"
                else
                    # Fallback for older CPUs -> Keel will probabky be slower
                    ARTIFACT="keel-x86_64-linux-v1"
                fi
                ;;
            aarch64) ARTIFACT="keel-aarch64-linux" ;;
            *)       error "Unsupported Linux architecture: $ARCH" ;;
        esac
        ;;
    *)
        # Windows will eventually be supported by an installer
        error "Unsupported OS: $OS. On Windows, download the .zip from https://github.com/horacehoff/keel/releases/latest"
        ;;
esac

print "Ground Control to Major Tom..."
print "Downloading $ARTIFACT for $OS/$ARCH"

TMP=$(mktemp -d)

# Clean up the temp directory once the script exits, for ANY reason
trap 'rm -rf "$TMP"' EXIT

$DOWNLOAD_CMD "https://github.com/horacehoff/keel/releases/latest/download/$ARTIFACT.tar.gz" | tar -xz -C "$TMP"

if [ ! -f "$TMP/keel" ]; then
    # The github workflow packs the binary straight into an archive so something went very wrong here
    error "Archive downloaded but binary not found inside. Please file a bug at https://github.com/horacehoff/keel/issues"
fi

if install "$TMP/$ARTIFACT" "$INSTALL_DIR/keel" 2>/dev/null; then
    :
elif command -v sudo >/dev/null 2>&1; then
    sudo install -m755 "$TMP/keel" "$INSTALL_DIR/keel"
else
    error "Cannot write to $INSTALL_DIR and sudo is not available. Re-run as root or install sudo."
fi

printf "${GREEN}[Keel]${RESET} Installed $("$INSTALL_DIR/keel" --version) in $INSTALL_DIR/keel\n"
printf "${GREEN}[Keel]${RESET} Run 'keel' to get started.\n"