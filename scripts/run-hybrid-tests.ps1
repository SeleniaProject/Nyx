# Runs nyx-crypto hybrid-handshake tests with an optional name filter.
# Usage examples (PowerShell):
#   .\scripts\run-hybrid-tests.ps1
#   .\scripts\run-hybrid-tests.ps1 -Filter test_key_pair_generation
#   .\scripts\run-hybrid-tests.ps1 -Filter test_complete_handshake_protocol

[CmdletBinding()] param(
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"

Write-Host "Running nyx-crypto tests with 'hybrid-handshake' feature..." -ForegroundColor Cyan

$cargoArgs = @("test", "-p", "nyx-crypto", "--features", "hybrid-handshake")
if ($Filter -ne "") {
    $cargoArgs += $Filter
}
$cargoArgs += "--"
$cargoArgs += "--nocapture"

Write-Host ("cargo " + ($cargoArgs -join ' ')) -ForegroundColor DarkGray

& cargo @cargoArgs
$exit = $LASTEXITCODE
if ($exit -ne 0) {
    Write-Error "Tests failed with exit code $exit"
    exit $exit
}

Write-Host "Done." -ForegroundColor Green
