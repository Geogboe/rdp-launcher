param(
    [string]$Target = "127.0.0.1",
    [int]$Seconds = 2
)

$rdpPath = Join-Path $env:TEMP "rdp-launch-smoke.rdp"

try {
    Set-Content -Path $rdpPath -Value "full address:s:$Target"
    $process = Start-Process -FilePath "mstsc.exe" -ArgumentList $rdpPath -PassThru
    Start-Sleep -Seconds $Seconds

    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
    }
}
finally {
    Remove-Item -Path $rdpPath -ErrorAction SilentlyContinue
}
