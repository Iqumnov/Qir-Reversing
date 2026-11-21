# ==============================================================================
# qir-reversing Automated Windows Toolchain Installer
# Downloads pre-compiled standalone binaries (qir.exe, qir-mcp.exe)
# Repository: https://github.com/Iqumnov/Qir-Reversing
# Author: Matvei Igumnov <matvei@iqumnov.com>
# License: Apache-2.0
# ==============================================================================

[CmdletBinding()]
param(
    [string]$Version = "v0.1.0",
    [string]$InstallDir = "$HOME\.qir\bin"
)

$ErrorActionPreference = "Stop"

Write-Host "================================================================" -ForegroundColor White
Write-Host "  qir-reversing // Windows Automated Toolchain Installer        " -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor White
Write-Host ""

$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($Arch -ne [System.Runtime.InteropServices.Architecture]::X64) {
    Write-Warning "Detected architecture: $Arch. Only x86_64 Windows is currently officially supported for prebuilt binaries."
}

if (-not (Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$ZipUrl = "https://github.com/Iqumnov/Qir-Reversing/releases/download/$Version/qir-windows-x86_64.zip"
$TempZip = Join-Path $env:TEMP "qir-windows-x86_64.zip"

Write-Host "[+] Downloading qir-reversing ($Version) for Windows x86_64..." -ForegroundColor Green
try {
    Invoke-WebRequest -Uri $ZipUrl -OutFile $TempZip -UseBasicParsing
    Expand-Archive -Path $TempZip -DestinationPath $InstallDir -Force
    Remove-Item -Path $TempZip -Force
    Write-Host "[✓] Download and extraction complete." -ForegroundColor Green
} catch {
    Write-Host "[!] Prebuilt binary download failed: $_" -ForegroundColor Yellow
    Write-Host "[*] Attempting fallback to cargo install..." -ForegroundColor Cyan
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        cargo install --git https://github.com/Iqumnov/Qir-Reversing qir-cli qir-mcp --force
    } else {
        Write-Error "Cargo is not installed and GitHub Release asset could not be downloaded."
    }
}

# Update User PATH if needed
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
    Write-Host "[✓] Added $InstallDir to user PATH." -ForegroundColor Green
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  qir-reversing installation successful!" -ForegroundColor Green
Write-Host "  Available binaries:" -ForegroundColor White
Write-Host "    - qir.exe     : Quantum IR disassembler & verifier" -ForegroundColor White
Write-Host "    - qir-mcp.exe : Native Model Context Protocol server" -ForegroundColor White
Write-Host "================================================================" -ForegroundColor Green

