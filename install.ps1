# Cross-platform installation script for hal (PowerShell)
# Detects OS and installs Rust if needed, then installs hal

$ErrorActionPreference = "Stop"

# Get the directory where this script is located
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = $ScriptDir

Write-Host "Installing hal (Homelab Automation Layer)..." -ForegroundColor Cyan

# Check if Rust is installed
$rustInstalled = $false
try {
    $null = Get-Command rustc -ErrorAction Stop
    $null = Get-Command cargo -ErrorAction Stop
    $rustInstalled = $true
} catch {
    $rustInstalled = $false
}

if (-not $rustInstalled) {
    Write-Host "Rust is not installed. Installing Rust..." -ForegroundColor Yellow
    
    # Download and run rustup-init
    $rustupUrl = "https://win.rustup.rs/x86_64"
    $rustupPath = Join-Path $env:TEMP "rustup-init.exe"
    
    Write-Host "Downloading rustup-init..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupPath
    
    Write-Host "Installing Rust..." -ForegroundColor Yellow
    & $rustupPath -y
    
    # Add cargo to PATH for current session
    $env:Path += ";$env:USERPROFILE\.cargo\bin"
    
    Write-Host "✓ Rust installed successfully" -ForegroundColor Green
    Write-Host "Please restart your terminal or run: refreshenv" -ForegroundColor Yellow
} else {
    Write-Host "✓ Rust is already installed" -ForegroundColor Green
    # Ensure cargo is in PATH
    if ($env:Path -notlike "*\.cargo\bin*") {
        $env:Path += ";$env:USERPROFILE\.cargo\bin"
    }
}

# Determine installation directory
$InstallDir = "$env:USERPROFILE\.local\bin"
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$InstallPath = Join-Path $InstallDir "hal.ps1"

# Check if hal already exists
if (Test-Path $InstallPath) {
    $response = Read-Host "hal already exists at $InstallPath. Overwrite? (y/N)"
    if ($response -ne "y" -and $response -ne "Y") {
        Write-Host "Installation cancelled." -ForegroundColor Yellow
        exit 0
    }
    Remove-Item $InstallPath -Force
}

# Build hal
Write-Host "Building hal..." -ForegroundColor Cyan
Push-Location $ProjectDir
cargo build --release
Pop-Location

# Create a PowerShell wrapper script
$WrapperScript = @"
# Wrapper script for hal
`$env:HOMELAB_DIR = "$ProjectDir"
& "$ProjectDir\target\release\hal.exe" `$args
"@

$WrapperScript | Out-File -FilePath $InstallPath -Encoding UTF8

Write-Host "✓ hal installed to $InstallPath" -ForegroundColor Green

# Check if install directory is in PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host ""
    Write-Host "Warning: $InstallDir is not in your PATH" -ForegroundColor Yellow
    Write-Host "Add this directory to your PATH environment variable" -ForegroundColor Yellow
    Write-Host "Or run: `$env:Path += `";$InstallDir`"" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Installation complete!" -ForegroundColor Green
Write-Host "Try: hal ssh bellerophon" -ForegroundColor Cyan

