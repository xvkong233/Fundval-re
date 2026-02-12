#!/bin/bash

# Fundval 停止脚本

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# PID 文件目录
PID_DIR="./pids"

echo "=========================================="
echo "    Fundval 服务停止"
echo "=========================================="
echo ""

# 停止服务
stop_service() {
    local service=$1
    local pid_file="$PID_DIR/$service.pid"

    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo "停止 $service (PID: $pid)..."
            kill "$pid"
            rm -f "$pid_file"
            echo -e "${GREEN}✓ $service 已停止${NC}"
        else
            echo -e "${YELLOW}$service 未运行${NC}"
            rm -f "$pid_file"
        fi
    else
        echo -e "${YELLOW}$service 未运行${NC}"
    fi
}

# 强制停止服务
force_stop_service() {
    local service=$1
    local pid_file="$PID_DIR/$service.pid"

    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo "强制停止 $service (PID: $pid)..."
            kill -9 "$pid"
            rm -f "$pid_file"
            echo -e "${GREEN}✓ $service 已强制停止${NC}"
        fi
    fi
}

# 停止所有服务
stop_all() {
    stop_service "django"
    sleep 1

    stop_service "celery-beat"
    sleep 1

    stop_service "celery-worker"
    sleep 1

    # Redis 通常不需要停止（系统服务）
    # stop_service "redis"
}

# 强制停止所有服务
force_stop_all() {
    echo ""
    echo "强制停止所有服务..."
    force_stop_service "django"
    force_stop_service "celery-beat"
    force_stop_service "celery-worker"
}

# 清理 Celery 相关文件
cleanup() {
    echo ""
    read -p "是否清理 Celery 临时文件？(y/N): " clean
    clean=${clean:-N}

    if [[ $clean =~ ^[Yy]$ ]]; then
        echo "清理临时文件..."
        rm -f logs/celerybeat-schedule*
        echo -e "${GREEN}✓ 清理完成${NC}"
    fi
}

# 主流程
main() {
    stop_all

    # 检查是否有服务未停止
    sleep 2
    local still_running=false

    for service in django celery-worker celery-beat; do
        if [ -f "$PID_DIR/$service.pid" ]; then
            local pid=$(cat "$PID_DIR/$service.pid")
            if ps -p "$pid" > /dev/null 2>&1; then
                still_running=true
                break
            fi
        fi
    done

    if [ "$still_running" = true ]; then
        echo ""
        echo -e "${YELLOW}部分服务未正常停止${NC}"
        read -p "是否强制停止？(y/N): " force
        force=${force:-N}

        if [[ $force =~ ^[Yy]$ ]]; then
            force_stop_all
        fi
    fi

    cleanup

    echo ""
    echo "=========================================="
    echo "    服务已停止"
    echo "=========================================="
    echo ""
}

main
