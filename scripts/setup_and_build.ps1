# ============================================================================
# Oxygen Browser — Setup, Environment Diagnostics & Build Script
# ============================================================================

[CmdletBinding()]
param(
    [switch]$BuildRelease = $false,
    [switch]$RunTests = $false,
    [switch]$CheckOnly = $false
)

$ErrorActionPreference = "Continue"

Write-Host "`n=======================================================" -ForegroundColor Cyan
Write-Host "   Oxygen Browser — Prerequisites & Diagnostic Check   " -ForegroundColor Cyan
Write-Host "=======================================================`n" -ForegroundColor Cyan

$allPassed = $true

# ----------------------------------------------------------------------------
# 1. Check Rust Toolchain
# ----------------------------------------------------------------------------
Write-Host "[1/3] Checking Rust Toolchain..." -NoNewline
try {
    $cargoVer = cargo --version 2>$null
    $rustcVer = rustc --version 2>$null
    if ($cargoVer -and $rustcVer) {
        Write-Host " [OK]" -ForegroundColor Green
        Write-Host "      Detected: $rustcVer" -ForegroundColor DarkGray
    } else {
        throw "Rust not detected"
    }
} catch {
    Write-Host " [FAIL]" -ForegroundColor Red
    Write-Host "      Rust is required to build Oxygen." -ForegroundColor Yellow
    Write-Host "      Please install Rust via: https://rustup.rs/" -ForegroundColor Yellow
    $allPassed = $false
}

# ----------------------------------------------------------------------------
# 2. Check Microsoft Edge WebView2 Evergreen Runtime
# ----------------------------------------------------------------------------
Write-Host "`n[2/3] Checking Microsoft Edge WebView2 Runtime..." -NoNewline
$wv2Paths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-F301-4FA5-B5A1-658F10022CB4}",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-F301-4FA5-B5A1-658F10022CB4}"
)

$wv2Found = $false
foreach ($p in $wv2Paths) {
    if (Test-Path $p) {
        $ver = (Get-ItemProperty -Path $p -Name "pv" -ErrorAction SilentlyContinue).pv
        if ($ver) {
            $wv2Found = $true
            Write-Host " [OK]" -ForegroundColor Green
            Write-Host "      Detected WebView2 version: $ver" -ForegroundColor DarkGray
            break
        }
    }
}

if (-not $wv2Found) {
    # Check default folder fallback
    $edgeFolder = "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application"
    if (Test-Path $edgeFolder) {
        Write-Host " [OK] (Found in $edgeFolder)" -ForegroundColor Green
        $wv2Found = $true
    } else {
        Write-Host " [WARN]" -ForegroundColor Yellow
        Write-Host "      Microsoft Edge WebView2 was not found in standard registry locations." -ForegroundColor Yellow
        Write-Host "      Download the Evergreen Bootstrapper: https://go.microsoft.com/fwlink/p/?LinkId=2124703" -ForegroundColor Yellow
    }
}

# ----------------------------------------------------------------------------
# 3. Check FFmpeg (Optional for Live RTMP Streaming & Video Recording)
# ----------------------------------------------------------------------------
Write-Host "`n[3/3] Checking FFmpeg (Optional - for Live Streaming)..." -NoNewline
$ffmpegFound = $false
try {
    $ffmpegVer = ffmpeg -version 2>$null
    if ($ffmpegVer) {
        $firstLine = ($ffmpegVer -split "`n")[0]
        Write-Host " [OK]" -ForegroundColor Green
        Write-Host "      Detected: $firstLine" -ForegroundColor DarkGray
        $ffmpegFound = $true
    }
} catch {
    # check repo local directory
    $repoRoot = Split-Path -Parent $PSScriptRoot
    if (Test-Path "$repoRoot\ffmpeg.exe") {
        Write-Host " [OK] (Found local ffmpeg.exe)" -ForegroundColor Green
        $ffmpegFound = $true
    }
}

if (-not $ffmpegFound) {
    Write-Host " [OPTIONAL - NOT DETECTED]" -ForegroundColor Yellow
    Write-Host "      FFmpeg is not installed on PATH." -ForegroundColor DarkGray
    Write-Host "      Core browser functions will work normally without FFmpeg." -ForegroundColor DarkGray
    Write-Host "      To enable live RTMP streaming, download FFmpeg from: https://www.gyan.dev/ffmpeg/builds/" -ForegroundColor DarkGray
    Write-Host "      Place 'ffmpeg.exe' in your PATH or in the same folder as Oxygen.exe." -ForegroundColor DarkGray
}

Write-Host "`n-------------------------------------------------------" -ForegroundColor Cyan

if (-not $allPassed) {
    Write-Host "Setup aborted: essential prerequisites are missing." -ForegroundColor Red
    exit 1
}

if ($CheckOnly) {
    Write-Host "Diagnostic check completed." -ForegroundColor Green
    exit 0
}

# ----------------------------------------------------------------------------
# 4. Run tests if requested
# ----------------------------------------------------------------------------
if ($RunTests) {
    Write-Host "`nRunning workspace unit tests (cargo test -p lightbrowser-core)..." -ForegroundColor Cyan
    $repoRoot = Split-Path -Parent $PSScriptRoot
    Push-Location $repoRoot
    try {
        cargo test -p lightbrowser-core
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Unit tests failed!" -ForegroundColor Red
            Pop-Location
            exit $LASTEXITCODE
        }
    } finally {
        Pop-Location
    }
}

# ----------------------------------------------------------------------------
# 5. Build Release Binary if requested
# ----------------------------------------------------------------------------
if ($BuildRelease) {
    Write-Host "`nBuilding Oxygen in release mode..." -ForegroundColor Cyan
    $repoRoot = Split-Path -Parent $PSScriptRoot
    Push-Location $repoRoot
    try {
        cargo build -p lightbrowser --bin Oxygen --release
        if ($LASTEXITCODE -eq 0) {
            Write-Host "`n[SUCCESS] Build complete! Binary located at: target\release\Oxygen.exe" -ForegroundColor Green
        } else {
            Write-Host "`n[FAIL] Build failed with exit code $LASTEXITCODE" -ForegroundColor Red
            Pop-Location
            exit $LASTEXITCODE
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "`nAll prerequisites are satisfied!" -ForegroundColor Green
    Write-Host "To build Oxygen, run:" -ForegroundColor White
    Write-Host "   cargo build -p lightbrowser --bin Oxygen --release`n" -ForegroundColor Cyan
}

exit 0
