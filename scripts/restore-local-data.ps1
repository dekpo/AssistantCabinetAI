# Restore data/ from a backup folder created by backup-local-data.ps1
# Usage: .\scripts\restore-local-data.ps1 backups\2026-09-15-160000
# Replaces current data/. Run from the repository root.

param(
    [Parameter(Mandatory = $true)]
    [string]$BackupDir
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "compose.yaml"))) {
    $Root = Get-Location
}

$Source = Join-Path $Root $BackupDir
if (-not (Test-Path (Join-Path $Source "data"))) {
    $Source = $BackupDir
}
if (-not (Test-Path (Join-Path $Source "data"))) {
    Write-Error "No data/ inside $BackupDir"
}

Set-Location $Root
Write-Host "Stopping stack..."
docker compose stop

$Data = Join-Path $Root "data"
if (Test-Path $Data) {
    Remove-Item -Recurse -Force $Data
}
Copy-Item -Path (Join-Path $Source "data") -Destination $Data -Recurse -Force

Write-Host "Starting stack..."
docker compose up -d

Write-Host "Restored from $Source"
