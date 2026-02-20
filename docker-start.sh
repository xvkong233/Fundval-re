#!/bin/bash
set -e

echo "=========================================="
echo "  Fundval - Docker Deployment"
echo "=========================================="
echo ""

# 优先使用 v2: `docker compose`，否则回退 v1: `docker-compose`
compose() {
    if docker compose version >/dev/null 2>&1; then
        docker compose "$@"
    else
        docker-compose "$@"
    fi
}

# 从 .env 读取指定变量（若不存在则返回空）
get_env_from_file() {
    local key="$1"
    if [ ! -f .env ]; then
        return 0
    fi
    # 取最后一次出现的 KEY=...；忽略注释/空行；保留等号右侧原样（包含冒号/引号等）
    sed -n -E "s/^[[:space:]]*${key}[[:space:]]*=[[:space:]]*(.*)[[:space:]]*$/\\1/p" .env | tail -n 1
}

# 获取最终端口：.env > 当前环境变量 > 默认值
resolve_port() {
    local key="$1"
    local default_value="$2"
    local from_file
    from_file="$(get_env_from_file "$key")"
    if [ -n "$from_file" ]; then
        echo "$from_file"
        return 0
    fi

    local from_env="${!key}"
    if [ -n "$from_env" ]; then
        echo "$from_env"
        return 0
    fi

    echo "$default_value"
}

# 检查 .env 文件
if [ ! -f .env ]; then
    echo "⚠ .env file not found, creating from .env.example..."
    cp .env.example .env
    echo "✓ .env created"
    echo ""
    echo "IMPORTANT: Edit .env and set secure passwords before production use!"
    echo ""
fi

# 构建并启动服务
echo "Building and starting services..."
compose up -d --build --remove-orphans

echo ""
echo "Waiting for services to be ready..."
sleep 5

# 提示：如果你同时运行了多个 compose 项目（例如本仓库的 git worktree），
# 可能会出现“访问了旧端口/旧容器”的错觉。先用 `docker compose ls` / `docker ps` 确认端口映射。
echo ""
echo "=========================================="
echo "  Compose Projects (Running)"
echo "=========================================="
docker compose ls 2>/dev/null || true

# 显示 bootstrap key（backend 会在未初始化时输出）
echo ""
echo "=========================================="
echo "  Getting Bootstrap Key"
echo "=========================================="
compose logs backend | grep -A 2 "BOOTSTRAP KEY" || echo "Waiting for backend to start..."

echo ""
echo "=========================================="
echo "  Deployment Complete!"
echo "=========================================="
echo ""

FRONTEND_HOST_PORT="$(resolve_port FRONTEND_HOST_PORT 3000)"
BACKEND_HOST_PORT="$(resolve_port BACKEND_HOST_PORT 8001)"

echo "Access the application at: http://localhost:$FRONTEND_HOST_PORT"
echo ""
echo "API endpoint: http://localhost:$BACKEND_HOST_PORT"
echo ""
echo "To view bootstrap key:"
echo "  $(docker compose version >/dev/null 2>&1 && echo "docker compose" || echo "docker-compose") logs backend | grep 'BOOTSTRAP KEY'"
echo ""
echo "To view logs:"
echo "  $(docker compose version >/dev/null 2>&1 && echo "docker compose" || echo "docker-compose") logs -f [service]"
echo ""
echo "To stop services:"
echo "  $(docker compose version >/dev/null 2>&1 && echo "docker compose" || echo "docker-compose") down"
echo ""
