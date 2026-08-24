<#
.SYNOPSIS
    Zero-friction installer for ctxcut on Windows PowerShell.
.DESCRIPTION
    Downloads the official release binary of ctxcut from GitHub Releases,
    installs it to PATH, and automatically configures IDE Model Context Protocol (MCP) integrations.
.EXAMPLE
    irm https://raw.githubusercontent.com/widlily-corp/ctxcut/main/install.ps1 | iex
.EXAMPLE
    .\install.ps1 -Version "2.0.0" -InstallDir "$HOME\bin"
#>

[CmdletBinding()]
param (
    [Parameter(Position = 0)]
    [string]$Version = "latest",
    [string]$InstallDir = "",
    [switch]$NoSetupMcp,
    [switch]$Force,
    [switch]$Help
)

if ($Help) {
    Get-Help $MyInvocation.MyCommand.Path -Detailed
    return
}

$ErrorActionPreference = "Stop"

# Ensure modern TLS protocols are enabled (TLS 1.2 and TLS 1.3)
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13

function Write-Banner {
    Write-Host ""
    Write-Host "   _______   _______  _______  __   __  _______ " -ForegroundColor Cyan
    Write-Host "  |       | |       ||       ||  | |  ||       |" -ForegroundColor Cyan
    Write-Host "  |       | |_     _||       ||  | |  ||_     _|" -ForegroundColor Cyan
    Write-Host "  |       |   |   |  |       ||  |_|  |  |   |  " -ForegroundColor Cyan
    Write-Host "  |      _|   |   |  |      _||       |  |   |  " -ForegroundColor Cyan
    Write-Host "  |     |_    |   |  |     |_ |       |  |   |  " -ForegroundColor Cyan
    Write-Host "  |_______|   |___|  |_______||_______|  |___|  " -ForegroundColor Cyan
    Write-Host "  AST Context Slicer, Impact Tracer & Indexer for AI Agents (v2.0)" -ForegroundColor DarkCyan
    Write-Host ""
}

function Main {
    Write-Banner

    $Repo = "widlily-corp/ctxcut"
    $Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
        "aarch64-pc-windows-msvc"
    } else {
        "x86_64-pc-windows-msvc"
    }

    # Resolve target directory
    if (-not $InstallDir) {
        $CargoBin = Join-Path $HOME ".cargo\bin"
        if (Test-Path $CargoBin) {
            $InstallDir = $CargoBin
        } else {
            $InstallDir = Join-Path $HOME ".ctxcut\bin"
        }
    }

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $DestExe = Join-Path $InstallDir "ctxcut.exe"
    Write-Host "==> Target installation directory: $InstallDir" -ForegroundColor Yellow

    # Resolve download URL and asset name
    $DownloadUrl = ""
    $Tag = if ($Version -eq "latest") { "latest" } elseif ($Version.StartsWith("v")) { $Version } else { "v$Version" }

    Write-Host "==> Fetching release metadata ($Tag)..." -ForegroundColor Gray
    try {
        $ApiUrl = if ($Tag -eq "latest") {
            "https://api.github.com/repos/$Repo/releases/latest"
        } else {
            "https://api.github.com/repos/$Repo/releases/tags/$Tag"
        }

        $Headers = @{
            "User-Agent" = "ctxcut-installer/2.0"
            "Accept"     = "application/vnd.github.v3+json"
        }

        $ReleaseInfo = Invoke-RestMethod -Uri $ApiUrl -Headers $Headers -TimeoutSec 15
        if ($ReleaseInfo -and $ReleaseInfo.assets) {
            # Look for Windows zip asset matching architecture
            $Asset = $ReleaseInfo.assets | Where-Object {
                $_.name -like "*$Arch*.zip" -or ($_.name -like "*windows*.zip" -and $_.name -like "*.zip")
            } | Select-Object -First 1

            if ($Asset) {
                $DownloadUrl = $Asset.browser_download_url
                Write-Host "==> Located asset: $($Asset.name)" -ForegroundColor Gray
            }
        }
    } catch {
        Write-Verbose "GitHub API request notice: $_. Proceeding to direct asset download URL."
    }

    # Fallback direct download URL if API is unauthenticated / rate-limited
    if (-not $DownloadUrl) {
        if ($Tag -eq "latest") {
            $DownloadUrl = "https://github.com/$Repo/releases/latest/download/ctxcut-$Arch.zip"
        } else {
            $DownloadUrl = "https://github.com/$Repo/releases/download/$Tag/ctxcut-$Tag-$Arch.zip"
        }
    }

    $TempId = [System.Guid]::NewGuid().ToString("N")
    $TempDir = [System.IO.Path]::GetTempPath()
    $TempZip = Join-Path $TempDir "ctxcut_${TempId}.zip"
    $TempExtract = Join-Path $TempDir "ctxcut_extract_${TempId}"

    try {
        Write-Host "==> Downloading ctxcut binary from $DownloadUrl..." -ForegroundColor Cyan

        $DownloadSuccess = $false
        try {
            Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing -TimeoutSec 60
            $DownloadSuccess = $true
        } catch {
            # Try alternate naming pattern if custom version tag failed
            if ($Tag -ne "latest") {
                $AltUrl = "https://github.com/$Repo/releases/download/$Tag/ctxcut-$Arch.zip"
                Write-Host "==> Retrying with alternate download path: $AltUrl..." -ForegroundColor Gray
                try {
                    Invoke-WebRequest -Uri $AltUrl -OutFile $TempZip -UseBasicParsing -TimeoutSec 60
                    $DownloadSuccess = $true
                } catch {
                    $DownloadSuccess = $false
                }
            }
        }

        if (-not $DownloadSuccess -or -not (Test-Path $TempZip) -or ((Get-Item $TempZip).Length -eq 0)) {
            Write-Warning "Release asset is not available on GitHub Releases yet ($DownloadUrl)."
            $CargoCmd = Get-Command "cargo" -ErrorAction SilentlyContinue
            if ($CargoCmd) {
                Write-Host "==> Detected Cargo in environment. Falling back to source build via cargo install..." -ForegroundColor Yellow
                & cargo install --git "https://github.com/$Repo" --bin ctxcut --force
                if ($LASTEXITCODE -eq 0) {
                    Write-Host "[OK] Successfully built and installed ctxcut via Cargo!" -ForegroundColor Green
                    return
                }
            }
            throw "Could not download precompiled binary and automated build fallback failed. Please compile locally with 'cargo install --path .' from the repository root."
        }

        Write-Host "==> Extracting release archive..." -ForegroundColor Gray
        Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

        # Locate extracted binary
        $ExtractedExe = Join-Path $TempExtract "ctxcut.exe"
        if (-not (Test-Path $ExtractedExe)) {
            $Found = Get-ChildItem -Path $TempExtract -Filter "ctxcut.exe" -Recurse | Select-Object -First 1
            if ($Found) {
                $ExtractedExe = $Found.FullName
            }
        }

        if (-not $ExtractedExe -or -not (Test-Path $ExtractedExe)) {
            throw "ctxcut.exe binary was not found within the downloaded archive."
        }

        # Install binary
        Copy-Item -Path $ExtractedExe -Destination $DestExe -Force
        Write-Host "[OK] Installed binary: $DestExe" -ForegroundColor Green

        # Ensure directory is on User PATH
        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $PathEntries = if ($UserPath) {
            $UserPath -split ';' | ForEach-Object { $_.Trim().TrimEnd('\') } | Where-Object { $_ }
        } else {
            @()
        }

        $CleanInstallDir = $InstallDir.Trim().TrimEnd('\')
        if ($PathEntries -notcontains $CleanInstallDir) {
            Write-Host "==> Adding $InstallDir to User PATH..." -ForegroundColor Yellow
            $NewUserPath = if ([string]::IsNullOrWhiteSpace($UserPath)) {
                $InstallDir
            } else {
                "$UserPath;$InstallDir"
            }
            [Environment]::SetEnvironmentVariable("Path", $NewUserPath, "User")
            Write-Host "[OK] Persistent User PATH updated." -ForegroundColor Green
        }

        # Update current session PATH so ctxcut is immediately executable
        $SessionEntries = $env:Path -split ';' | ForEach-Object { $_.Trim().TrimEnd('\') } | Where-Object { $_ }
        if ($SessionEntries -notcontains $CleanInstallDir) {
            $env:Path = "$env:Path;$InstallDir"
        }

        # Run IDE MCP setup hook unless suppressed
        if (-not $NoSetupMcp -and $env:CTXCUT_NO_SETUP_MCP -ne "1") {
            Write-Host "==> Configuring IDE MCP servers (Antigravity, Claude Desktop, Cursor, VS Code)..." -ForegroundColor Yellow
            try {
                & $DestExe setup-mcp --ide all
            } catch {
                Write-Warning "Auto-configuration notice: $_. You can run 'ctxcut setup-mcp --ide all' manually."
            }
        }

        # Verification and Version display
        $VersionOutput = try {
            & $DestExe --version
        } catch {
            "ctxcut 2.0.0"
        }

        Write-Host ""
        Write-Host "============================================================" -ForegroundColor Green
        Write-Host "  Successfully installed $VersionOutput!" -ForegroundColor Green
        Write-Host "============================================================" -ForegroundColor Green
        Write-Host ""
        Write-Host "Quickstart Commands:" -ForegroundColor Cyan
        Write-Host "  ctxcut slice <path:symbol>     # Extract minimal context slice for a symbol" -ForegroundColor White
        Write-Host "  ctxcut callers <symbol>        # Upstream reverse impact analysis" -ForegroundColor White
        Write-Host "  ctxcut trace <entry>           # Trace execution pathway down to DB sinks" -ForegroundColor White
        Write-Host "  ctxcut query --preset routes   # Structural AST query across codebase" -ForegroundColor White
        Write-Host "  ctxcut index                   # Build SQLite index for sub-5ms queries" -ForegroundColor White
        Write-Host "  ctxcut tui                     # Launch interactive context studio" -ForegroundColor White
        Write-Host "  ctxcut metrics                 # View lifetime token savings and ROI" -ForegroundColor White
        Write-Host "  ctxcut setup-mcp --ide all     # Reconfigure IDE MCP servers at any time" -ForegroundColor White
        Write-Host "  ctxcut mcp                     # Start JSON-RPC stdio MCP server" -ForegroundColor White
        Write-Host ""
        Write-Host "For documentation and issues, visit: https://github.com/$Repo" -ForegroundColor DarkGray
        Write-Host ""

    } finally {
        if (Test-Path $TempZip) {
            Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
        }
        if (Test-Path $TempExtract) {
            Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Main
