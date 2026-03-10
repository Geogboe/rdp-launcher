$vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
$installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Workload.VCTools -property installationPath

if (-not $installationPath) {
    throw "Visual Studio Build Tools with the VCTools workload was not found."
}

$vsDevCmd = Join-Path $installationPath "Common7\Tools\VsDevCmd.bat"

if (-not (Test-Path $vsDevCmd)) {
    throw "VsDevCmd.bat was not found at $vsDevCmd"
}

cmd.exe /c "`"$vsDevCmd`" -arch=arm64 -host_arch=x64 && where link.exe && set VCToolsInstallDir && set WindowsSdkDir"
