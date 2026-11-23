$INSTALL_DIR = Join-Path $env:USERPROFILE ".keel"
$OLD_INSTALL_DIR = Join-Path $env:LOCALAPPDATA "Keel"

$ErrorActionPreference = 'Stop'
If(!(test-path -PathType container $INSTALL_DIR))
{
      New-Item -ItemType Directory -Path $INSTALL_DIR
}

Write-Host "[Keel] Ground Control to Major Tom..."
Write-Host "[Keel] Downloading Keel..."

$ARCHIVE_PATH = Join-Path $INSTALL_DIR "keel-x86_64-windows.zip"
Invoke-WebRequest "https://github.com/horacehoff/keel/releases/latest/download/keel-x86_64-windows.zip" -OutFile $ARCHIVE_PATH
Expand-Archive -Force -Path $ARCHIVE_PATH -DestinationPath $INSTALL_DIR
Remove-Item -Force -Path $ARCHIVE_PATH

# Add it to PATH only if it's not already in there
$paths = @([Environment]::GetEnvironmentVariable("Path", "User") -split [IO.Path]::PathSeparator | Where-Object {$_ -and $_ -ne $OLD_INSTALL_DIR -and $_ -ne $INSTALL_DIR})
[Environment]::SetEnvironmentVariable("Path",(($paths + $INSTALL_DIR) -join [IO.Path]::PathSeparator), "User")
if (Test-Path $OLD_INSTALL_DIR -PathType Container) {
    Remove-Item $OLD_INSTALL_DIR -Recurse -Force
}

$exe_path = Join-Path $INSTALL_DIR "keel.exe"
$keel_version = & $exe_path --version 2>&1
Write-Host "[Keel] Installed $keel_version in $INSTALL_DIR and added it to PATH."
Write-Host "[Keel] Restart your terminal, then run 'keel' to get started."
