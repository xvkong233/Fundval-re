param(
  [Parameter(Position = 0)]
  [ValidateSet("up", "down", "status", "logs")]
  [string]$Command = "up",

  [string]$ProjectName = "fundval-dev",
  [string]$FrontendPort = "3000",
  [string]$BackendPort = "8001",
  [string]$QuantPort = "8002",
  [switch]$Build,
  [string[]]$Services = @("db-candidate", "quant-service", "backend", "frontend")
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
$env:QUANT_HOST_PORT = $QuantPort

if ($Command -eq "down") {
  Invoke-Compose @("down", "--remove-orphans")
  exit 0
}

if ($Command -eq "status") {
  Invoke-Compose @("ps")
  exit 0
}

if ($Command -eq "logs") {
  Invoke-Compose @("logs", "-f")
  exit 0
}

# up: 先清旧容器（不删 volume），再启动；避免堆积容器/网络
try {
  Invoke-Compose @("down", "--remove-orphans")
} catch {
  # ignore: 项目不存在时 down 可能返回非 0（不同版本/环境行为不一致）
}

$upArgs = @("up", "-d")
if ($Build) { $upArgs += "--build" } else { $upArgs += "--no-build" }
$upArgs += $Services

Invoke-Compose $upArgs

  Write-Host ""
  Write-Host "✅ 已启动 (project=$ProjectName)"
  Write-Host "   - Backend API: http://localhost:$BackendPort/api/health/"
  Write-Host "   - Quant API  : http://localhost:$QuantPort/health"
  Write-Host "   - Frontend  : http://localhost:$FrontendPort/"
