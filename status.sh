#!/bin/bash

# Fundval 服务状态查看

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

PID_DIR="./pids"

echo "=========================================="
echo "    Fundval 服务状态"
echo "=========================================="
echo ""

check_service() {
    local service=$1
    local pid_file="$PID_DIR/$service.pid"

    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if ps -p "$pid" > /dev/null 2>&1; then
            echo -e "$service: ${GREEN}运行中${NC} (PID: $pid)"
        else
            echo -e "$service: ${RED}未运行${NC} (PID 文件存在但进程不存在)"
        fi
    else
        echo -e "$service: ${RED}未运行${NC}"
    fi
}

check_service "Redis"
check_service "Celery Worker"
check_service "Celery Beat"
check_service "Django"

echo ""
echo "查看日志:"
echo "  tail -f logs/django.log"
echo "  tail -f logs/celery-worker.log"
echo "  tail -f logs/celery-beat.log"
echo ""
