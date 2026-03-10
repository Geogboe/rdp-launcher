param(
    [switch]$Release,
    [switch]$Build
)

$target = "aarch64-pc-windows-msvc"
$profile = if ($Release) { "release" } else { "debug" }
$exe = Join-Path $env:LOCALAPPDATA "rdp-launch\target\$target\$profile\rdp-launch-desktop.exe"
$buildScript = Join-Path $PSScriptRoot "build-windows-desktop.ps1"

if ($Build) {
    $buildArgs = @()
    if ($Release) {
        $buildArgs += "-Release"
    }
    & $buildScript @buildArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Desktop build failed with exit code $LASTEXITCODE"
    }
}

if (-not (Test-Path $exe)) {
    throw "Desktop binary not found at $exe"
}

Start-Process -FilePath $exe
