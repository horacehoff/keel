$INSTALL_DIR = "$env:LOCALAPPDATA\Keel"

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
$user_path =  [Environment]::GetEnvironmentVariable("PATH", "User")
if (!($user_path -split [IO.Path]::PathSeparator -contains $INSTALL_DIR)) {
    $Path = $user_path + [IO.Path]::PathSeparator + $INSTALL_DIR
    [Environment]::SetEnvironmentVariable( "Path", $Path, "User")
}

$exe_path = Join-Path $INSTALL_DIR "keel.exe"
$keel_version = & $exe_path --version 2>&1
Write-Host "[Keel] Installed $keel_version in $INSTALL_DIR and added it to PATH."
Write-Host "[Keel] Restart your terminal, then run 'keel' to get started."
