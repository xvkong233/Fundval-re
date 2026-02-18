param(
  [string]$ProjectName = "fundval-check-funds",
  [string]$FrontendPort = "3000",
  [string]$BackendPort = "8001",
  [switch]$Build,
  [string]$FundCode = ""
)

$ErrorActionPreference = "Stop"

function Invoke-Compose([string[]]$ComposeArgs) {
  $projectDir = Resolve-Path (Join-Path $PSScriptRoot "..")
  docker compose --project-directory $projectDir -p $ProjectName @ComposeArgs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$env:COMPOSE_PROJECT_NAME = $ProjectName
$env:FRONTEND_HOST_PORT = $FrontendPort
$env:BACKEND_HOST_PORT = $BackendPort

# 启动（不删 volume）
$upArgs = @("up", "-d")
if ($Build) { $upArgs += "--build" } else { $upArgs += "--no-build" }
$upArgs += @("db-candidate", "backend", "frontend")
Invoke-Compose $upArgs

$healthUrl = "http://localhost:$BackendPort/api/health/"
Write-Host "Waiting for API health: $healthUrl"
for ($i = 0; $i -lt 90; $i++) {
  try {
    $r = Invoke-RestMethod -Method Get -Uri $healthUrl -TimeoutSec 3
    if ($null -ne $r.status) { break }
  } catch {
    Start-Sleep -Seconds 1
  }
}

try {
  $health = Invoke-RestMethod -Method Get -Uri $healthUrl -TimeoutSec 5
  Write-Host "✅ health ok: status=$($health.status) database=$($health.database) initialized=$($health.system_initialized)"
} catch {
  Write-Host "❌ health failed: $($_.Exception.Message)"
  exit 1
}

$fundsUrl = "http://localhost:$BackendPort/api/funds/?page=1&page_size=1"
Write-Host "Checking funds list: $fundsUrl"
try {
  $funds = Invoke-RestMethod -Method Get -Uri $fundsUrl -TimeoutSec 10
  if ($null -eq $funds.count -or $null -eq $funds.results) {
    throw "unexpected response shape"
  }
  Write-Host "✅ funds list ok: count=$($funds.count) results_len=$($funds.results.Count)"
} catch {
  Write-Host "❌ funds list failed: $($_.Exception.Message)"
  exit 1
}

if ($FundCode -and $FundCode.Trim().Length -gt 0) {
  $detailUrl = "http://localhost:$BackendPort/api/funds/$FundCode/"
  Write-Host "Checking fund detail: $detailUrl"
  try {
    $detail = Invoke-RestMethod -Method Get -Uri $detailUrl -TimeoutSec 10
    Write-Host "✅ fund detail ok: fund_code=$($detail.fund_code) fund_name=$($detail.fund_name)"
  } catch {
    Write-Host "⚠ fund detail request failed (might be 404 if not synced): $($_.Exception.Message)"
  }
}

Write-Host ""
Write-Host "Done."
Write-Host " - Frontend: http://localhost:$FrontendPort/"
Write-Host " - API:      http://localhost:$BackendPort/"

