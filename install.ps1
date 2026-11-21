# ==============================================================================
# qir-reversing Automated Windows Toolchain & Plugin Installer
# Downloads pre-compiled standalone binaries (qir-rev.exe, qir-rev-mcp.exe)
# Automatically installs Rizin architecture plugin and Cutter Python GUI dock
# Repository: https://github.com/Iqumnov/Qir-Reversing
# Author: Matvei Igumnov <matvei@iqumnov.com>
# License: Apache-2.0
# ==============================================================================

[CmdletBinding()]
param(
    [string]$Version = "v0.1.0",
    [string]$InstallDir = "$HOME\.qir-rev\bin"
)

$ErrorActionPreference = "Stop"

Write-Host "================================================================" -ForegroundColor White
Write-Host "  qir-reversing // Windows Automated Toolchain & Plugin Setup   " -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor White
Write-Host ""

$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($Arch -ne [System.Runtime.InteropServices.Architecture]::X64) {
    Write-Warning "Detected architecture: $Arch. Only x86_64 Windows is currently officially supported for prebuilt binaries."
}

if (-not (Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$ZipUrls = @(
    "https://github.com/Iqumnov/Qir-Reversing/releases/download/$Version/qir-rev-windows-x86_64.zip",
    "https://github.com/Iqumnov/Qir-Reversing/releases/download/$Version/qir-windows-x86_64.zip"
)

$TempZip = Join-Path $env:TEMP "qir-rev-windows-x86_64.zip"
$TempExtract = Join-Path $env:TEMP "qir-rev-extract"

if (Test-Path $TempExtract) {
    Remove-Item -Path $TempExtract -Recurse -Force
}
New-Item -ItemType Directory -Path $TempExtract -Force | Out-Null

$DownloadSuccess = $false
foreach ($Url in $ZipUrls) {
    Write-Host "[+] Attempting download from: $Url" -ForegroundColor Green
    try {
        Invoke-WebRequest -Uri $Url -OutFile $TempZip -UseBasicParsing
        Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force
        Remove-Item -Path $TempZip -Force
        $DownloadSuccess = $true
        Write-Host "[✓] Download and extraction complete." -ForegroundColor Green
        break
    } catch {
        Write-Host "[!] Asset not found at $Url, trying alternative..." -ForegroundColor Yellow
    }
}

if (-not $DownloadSuccess) {
    Write-Host "[*] Attempting fallback to cargo install..." -ForegroundColor Cyan
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        cargo install --git https://github.com/Iqumnov/Qir-Reversing --bin qir-rev --bin qir-rev-mcp --force
        $InstallDir = "$HOME\.cargo\bin"
    } else {
        Write-Error "Cargo is not installed and GitHub Release assets could not be downloaded."
    }
} else {
    # 1. Install CLI & MCP Binaries
    $CliExe = Get-ChildItem -Path $TempExtract -Recurse -Filter "*qir*.exe" | Where-Object { $_.Name -match "qir(-rev)?\.exe$" } | Select-Object -First 1
    $McpExe = Get-ChildItem -Path $TempExtract -Recurse -Filter "*mcp*.exe" | Select-Object -First 1

    if ($CliExe) {
        Copy-Item -Path $CliExe.FullName -Destination (Join-Path $InstallDir "qir-rev.exe") -Force
        # Create convenient qrev.cmd wrapper
        $QrevCmd = Join-Path $InstallDir "qrev.cmd"
        Set-Content -Path $QrevCmd -Value "@echo off`r`n`"%~dp0\qir-rev.exe`" %*"
        # Create backward-compat qir.cmd wrapper
        $QirCmd = Join-Path $InstallDir "qir.cmd"
        Set-Content -Path $QirCmd -Value "@echo off`r`n`"%~dp0\qir-rev.exe`" %*"
        Write-Host "[✓] Installed qir-rev.exe (aliases: qrev, qir) to: $InstallDir" -ForegroundColor Green
    }

    if ($McpExe) {
        Copy-Item -Path $McpExe.FullName -Destination (Join-Path $InstallDir "qir-rev-mcp.exe") -Force
        Write-Host "[✓] Installed qir-rev-mcp.exe to: $InstallDir" -ForegroundColor Green
    }

    # 2. Install Rizin Native Plugin (rz_qir.dll) if present
    $RzDll = Get-ChildItem -Path $TempExtract -Recurse -Filter "*rz_qir.dll" | Select-Object -First 1
    if ($RzDll) {
        $RizinPluginDir = Join-Path $env:APPDATA "rizin\plugins"
        if (-not (Test-Path $RizinPluginDir)) {
            New-Item -ItemType Directory -Path $RizinPluginDir -Force | Out-Null
        }
        Copy-Item -Path $RzDll.FullName -Destination (Join-Path $RizinPluginDir "rz_qir.dll") -Force
        Write-Host "[✓] Installed Rizin architecture plugin to: $RizinPluginDir\rz_qir.dll" -ForegroundColor Green
    }

    # 3. Install Cutter GUI Python Plugin (qir_plugin) if present
    $PluginFolder = Get-ChildItem -Path $TempExtract -Recurse -Directory -Filter "qir_plugin" | Select-Object -First 1
    if ($PluginFolder) {
        $CutterPluginDir = Join-Path $env:APPDATA "rizin\cutter\plugins\python"
        if (-not (Test-Path $CutterPluginDir)) {
            New-Item -ItemType Directory -Path $CutterPluginDir -Force | Out-Null
        }
        $DestPluginDir = Join-Path $CutterPluginDir "qir_plugin"
        if (Test-Path $DestPluginDir) {
            Remove-Item -Path $DestPluginDir -Recurse -Force
        }
        Copy-Item -Path $PluginFolder.FullName -Destination $CutterPluginDir -Recurse -Force
        Write-Host "[✓] Installed Cutter GUI Python Dock to: $DestPluginDir" -ForegroundColor Green
    }

    # Cleanup temp extraction
    Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue
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
Write-Host "  qir-reversing deployment successful!" -ForegroundColor Green
Write-Host "  Available tools:" -ForegroundColor White
Write-Host "    - qir-rev.exe (or 'qrev') : Quantum IR disassembler & verifier" -ForegroundColor White
Write-Host "    - qir-rev-mcp.exe         : Native Model Context Protocol server" -ForegroundColor White
Write-Host "    - Rizin Native Plugin     : %APPDATA%\rizin\plugins\rz_qir.dll" -ForegroundColor White
Write-Host "    - Cutter GUI Dock Plugin  : %APPDATA%\rizin\cutter\plugins\python\qir_plugin" -ForegroundColor White
Write-Host "================================================================" -ForegroundColor Green
