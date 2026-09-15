# Copy local stack data (account, chats, prompts, models) to backups/<timestamp>.
# Does not upload anything. Does not use git. Run from the repository root.
# Stop Compose first so the Open WebUI database is not copied mid-write.

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "compose.yaml"))) {
    $Root = Get-Location
}

$Data = Join-Path $Root "data"
if (-not (Test-Path $Data)) {
    Write-Error "No data/ folder yet. Start the stack at least once, or migrate from named volumes (see docs/user/local-compose.md)."
}

$Stamp = Get-Date -Format "yyyy-MM-dd-HHmmss"
$Dest = Join-Path $Root "backups\$Stamp"
New-Item -ItemType Directory -Force -Path $Dest | Out-Null

Set-Location $Root
Write-Host "Stopping stack for a consistent copy..."
docker compose stop

Copy-Item -Path $Data -Destination (Join-Path $Dest "data") -Recurse -Force

Write-Host "Starting stack..."
docker compose up -d

Write-Host "Backup written to $Dest"
