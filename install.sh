#!/bin/sh

# KEEL INSTALLER

set -e

# Supported OS's: "Darwin" on macOS, "Linux" on Linux
OS=$(uname -s)

case "$OS" in
    Darwin) INSTALL_DIR="/Library/Keel/" ;;
    Linux) INSTALL_DIR="/usr/local/lib/keel/" ;;
esac

if mkdir -p "$INSTALL_DIR" 2>/dev/null; then
    :
elif command -v sudo >/dev/null 2>&1; then
    sudo mkdir $INSTALL_DIR
else
    printf "[ERROR] Cannot write to $INSTALL_DIR and sudo is not available. Re-run as root or install sudo.\n"
fi

if command -v curl >/dev/null 2>&1; then
    # Fail silently on HTTP errors & show errors even when silent & follow redirects & show progress bar
    DOWNLOAD_CMD="curl -fSL --progress-bar"
elif command -v wget >/dev/null 2>&1; then
    # Write output to stdout & show progress bar
    DOWNLOAD_CMD="wget -O- --show-progress"
else
    printf "[ERROR] curl or wget is required\n"
fi

# Supported archs: x86_64, arm64, aarch64
ARCH=$(uname -m)

case "$OS" in
    Darwin)
        case "$ARCH" in
            x86_64)  ARTIFACT="keel-x86_64-apple-darwin" ;;
            arm64)   ARTIFACT="keel-aarch64-apple-darwin" ;;
            *)       printf "[ERROR] Unsupported macOS architecture: $ARCH\n" ;;
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
            *)       printf "[ERROR] Unsupported Linux architecture: $ARCH\n" ;;
        esac
        ;;
    *)
        # Windows will eventually be supported by an installer
        printf "[ERROR] Unsupported OS: $OS. On Windows, download the .zip from https://github.com/horacehoff/keel/releases/latest\n"
        ;;
esac

printf "[Keel] Ground Control to Major Tom...\n"
printf "[Keel] Downloading $ARTIFACT for $OS/$ARCH\n"

TMP=$(mktemp -d)

# Clean up the temp directory once the script exits, for ANY reason
trap 'rm -rf "$TMP"' EXIT

$DOWNLOAD_CMD "https://github.com/horacehoff/keel/releases/latest/download/$ARTIFACT.tar.gz" | tar -xz -C "$TMP"

if [ ! -f "$TMP/keel" ]; then
    # The github workflow packs the binary straight into an archive so something went very wrong here
    printf "[ERROR] Archive downloaded but binary not found inside. Please file a bug report at https://github.com/horacehoff/keel/issues\n"
fi

if cp -R "$TMP/." "$INSTALL_DIR" 2>/dev/null; then
    :
elif command -v sudo >/dev/null 2>&1; then
    sudo cp -R "$TMP/." "$INSTALL_DIR"
else
    printf "[ERROR] Cannot write to $INSTALL_DIR and sudo is not available. Re-run as root or install sudo.\n"
fi

if chmod 755 "$INSTALL_DIR/keel" 2>/dev/null; then
    :
elif command -v sudo >/dev/null 2>&1; then
    sudo chmod 755 "$INSTALL_DIR/keel"
fi

if ln -sf "$INSTALL_DIR/keel" /usr/local/bin/keel 2>/dev/null; then
    :
elif command -v sudo >/dev/null 2>&1; then
    sudo ln -sf "$INSTALL_DIR/keel" /usr/local/bin/keel
else
    printf "[ERROR] Cannot write to /usr/local/bin and sudo is not available. Re-run as root or install sudo.\n"
fi

printf "[Keel] Installed $("$INSTALL_DIR/keel" --version) in $INSTALL_DIRkeel\n"
printf "[Keel] Run 'keel' to get started.\n"