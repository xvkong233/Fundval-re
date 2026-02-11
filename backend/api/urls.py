from django.urls import path
from . import views

urlpatterns = [
    # 系统管理
    path('health/', views.health, name='health'),

    # Bootstrap 初始化
    path('admin/bootstrap/verify', views.bootstrap_verify, name='bootstrap_verify'),
    path('admin/bootstrap/initialize', views.bootstrap_initialize, name='bootstrap_initialize'),

    # 认证
    path('auth/login', views.login, name='login'),
    path('auth/refresh', views.refresh_token, name='refresh_token'),
    path('auth/me', views.get_current_user, name='get_current_user'),
    path('auth/password', views.change_password, name='change_password'),
]
