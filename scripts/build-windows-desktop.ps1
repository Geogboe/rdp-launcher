param(
    [switch]$Release
)

$repoRoot = Split-Path -Parent $PSScriptRoot
$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"

if (-not (Test-Path $vswhere)) {
    throw "vswhere.exe was not found at $vswhere"
}

$installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath

if (-not $installationPath) {
    throw "Visual Studio Build Tools with the VCTools workload was not found."
}

$devShell = Join-Path $installationPath "Common7\Tools\Launch-VsDevShell.ps1"
$vsDevCmd = Join-Path $installationPath "Common7\Tools\VsDevCmd.bat"

if (-not (Test-Path $vsDevCmd)) {
    throw "VsDevCmd.bat was not found at $vsDevCmd"
}

$target = "aarch64-pc-windows-msvc"
$arguments = @("build", "-p", "rdp-launch-desktop", "--target", $target)
$windowsTargetDir = [System.IO.Path]::Combine($env:LOCALAPPDATA, "rdp-launch", "target")

if ($Release) {
    $arguments += "--release"
}

$manifestPath = Join-Path $repoRoot "Cargo.toml"
New-Item -ItemType Directory -Force -Path $windowsTargetDir | Out-Null
$cargoCommand = "cargo.exe " + (($arguments + @("--manifest-path", "`"$manifestPath`"")) -join " ")
$cmdLine = "`"$vsDevCmd`" -arch=arm64 -host_arch=x64 && set `"CARGO_INCREMENTAL=0`" && set `"CARGO_TARGET_DIR=$windowsTargetDir`" && $cargoCommand"

& cmd.exe /c $cmdLine
if ($LASTEXITCODE -ne 0) {
    throw "Windows desktop build failed with exit code $LASTEXITCODE"
}
