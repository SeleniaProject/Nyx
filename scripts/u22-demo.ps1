# U22 minimal demo script (Windows PowerShell)
# - Builds workspace (release)
# - Starts daemon in a new window
# - Prints CLI help and writes a template config if needed

$ErrorActionPreference = 'Stop'

Write-Host 'Building workspace (release)...'
cargo build --workspace --release

Write-Host 'Launching nyx-daemon in a new PowerShell window...'
# Start a new PowerShell process for the daemon so the script can continue
Start-Process -FilePath 'powershell.exe' -ArgumentList 'cargo run -p nyx-daemon --release' -NoNewWindow:$false

Start-Sleep -Seconds 2

Write-Host 'CLI help:'
cargo run -p nyx-cli --release -- --help

# Optionally create a config template at ./nyx.toml if not present
if (-not (Test-Path -Path './nyx.toml')) {
    Write-Host 'Writing nyx.toml template to current directory...'
    cargo run -p nyx-cli --release -- config write-template --path nyx.toml
}

Write-Host 'Done. You can set $env:NYX_CONFIG to point to nyx.toml if needed.'
