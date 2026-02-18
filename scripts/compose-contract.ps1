param(
  [Parameter(Position = 0)]
  [ValidateSet("run", "down", "status", "logs")]
  [string]$Command = "run",

  [string]$ProjectName = "fundval-contract",
  [switch]$Build
)

$ErrorActionPreference = "Stop"

function Invoke-Compose([string[]]$ComposeArgs) {
  $projectDir = Resolve-Path (Join-Path $PSScriptRoot "..")
  docker compose --project-directory $projectDir -p $ProjectName --profile contract @ComposeArgs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$env:COMPOSE_PROJECT_NAME = $ProjectName

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

# run: 先清旧容器（不删 volume），再执行合同测试；结束后再次 down，避免堆积容器/网络
try {
  Invoke-Compose @("down", "--remove-orphans")
} catch {
  # ignore: 项目不存在时 down 可能返回非 0（不同版本/环境行为不一致）
}

try {
  $args = @("up", "--abort-on-container-exit", "--exit-code-from", "contract-tests")
  if ($Build) { $args += "--build" }
  $args += @("contract-tests")

  Invoke-Compose $args
} finally {
  try {
    Invoke-Compose @("down", "--remove-orphans")
  } catch {
    # ignore
  }
}

