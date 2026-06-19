# pkg/windows/build.ps1 — Baut das ZeroDDS-MSI-Paket auf Windows.
#
# Voraussetzungen (im CI / Dev-Maschine):
#   * Rust toolchain 1.85+ (rustup default stable, target x86_64-pc-windows-msvc)
#   * .NET 8 SDK
#   * dotnet tool install -g wix --version 5.0.*
#   * Optional: signtool.exe (Windows SDK)
#
# Build im Repo-Root:
#   pwsh -File pkg/windows/build.ps1 -Configuration Release -Sign $false
#
# Mit Code-Signing:
#   pwsh -File pkg/windows/build.ps1 -Sign $true -CertSubject "ZeroDDS Maintainers"

[CmdletBinding()]
param(
    [string]$Configuration = "release",
    [bool]$Sign = $false,
    [string]$CertSubject = "",
    [string]$OutDir = "dist/windows"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Resolve-Path "$PSScriptRoot/../.."
Push-Location $RepoRoot

try {
    Write-Host "==> cargo build --release (Workspace-Tools + libs)"
    cargo build --release `
        -p dds-admin -p dds-perf -p dds-idlc -p dds-xmlc `
        -p dds-traceability -p dds-chaos `
        -p dds-bench-suite `
        -p dds-c-api
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    Write-Host "==> WiX-Build"
    if (-not (Test-Path $OutDir)) {
        New-Item -ItemType Directory -Path $OutDir | Out-Null
    }
    $wxs = Join-Path $RepoRoot "pkg/windows/zerodds.wxs"
    $msi = Join-Path $OutDir "zerodds-0.0.0-x64.msi"
    & wix build $wxs -arch x64 -o $msi
    if ($LASTEXITCODE -ne 0) { throw "wix build failed" }

    if ($Sign) {
        Write-Host "==> signtool sign /fd SHA256 /a $msi"
        & signtool sign /fd SHA256 /a /n $CertSubject /t http://timestamp.digicert.com $msi
        if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
    } else {
        Write-Host "==> Skipping code-signing (-Sign 0)"
    }

    Write-Host "==> Done: $msi"
} finally {
    Pop-Location
}
