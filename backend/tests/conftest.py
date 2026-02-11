import pytest
import os
import django
from django.conf import settings

# 配置 Django 设置
os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'fundval.settings')

def pytest_configure(config):
    """配置 pytest-django"""
    if not settings.configured:
        django.setup()
