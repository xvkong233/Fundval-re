from django.db import connection
from django.http import JsonResponse
from fundval.config import config


def health(request):
    """健康检查接口"""
    # 检查数据库连接
    db_status = 'disconnected'
    try:
        connection.ensure_connection()
        db_status = 'connected'
    except Exception:
        pass

    return JsonResponse({
        'status': 'ok',
        'database': db_status,
        'system_initialized': config.get('system_initialized', False),
    })
