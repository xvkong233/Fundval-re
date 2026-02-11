"""
测试配置读取模块

测试点：
1. 默认配置加载
2. JSON 文件配置加载
3. 环境变量覆盖
4. 配置保存
"""
import json
import os
import tempfile
from pathlib import Path
import pytest


class TestConfig:
    """配置读取测试"""

    def test_default_config(self):
        """测试默认配置"""
        from fundval.config import Config

        config = Config()
        assert config.get('port') == 8000
        assert config.get('db_type') == 'sqlite'
        assert config.get('allow_register') is False
        assert config.get('system_initialized') is False
        assert config.get('debug') is False

    def test_json_config_load(self, tmp_path):
        """测试 JSON 配置文件加载"""
        # 创建临时配置文件
        config_data = {
            'port': 9000,
            'db_type': 'postgresql',
            'allow_register': True,
        }
        config_file = tmp_path / 'config.json'
        with open(config_file, 'w') as f:
            json.dump(config_data, f)

        # TODO: 需要修改 Config 类支持自定义配置路径
        # 或者使用 monkeypatch 修改路径
        pass

    def test_env_override(self, monkeypatch):
        """测试环境变量覆盖配置"""
        monkeypatch.setenv('PORT', '9000')
        monkeypatch.setenv('DB_TYPE', 'postgresql')
        monkeypatch.setenv('ALLOW_REGISTER', 'true')
        monkeypatch.setenv('DEBUG', 'true')

        from fundval.config import Config

        # 重新加载配置
        Config._instance = None
        Config._config = None
        config = Config()

        assert config.get('port') == 9000
        assert config.get('db_type') == 'postgresql'
        assert config.get('allow_register') is True
        assert config.get('debug') is True

    def test_config_set_and_save(self, tmp_path):
        """测试配置修改和保存"""
        from fundval.config import Config

        config = Config()
        config.set('system_initialized', True)
        assert config.get('system_initialized') is True

        # TODO: 测试保存功能
        # config.save()
        pass

    def test_config_singleton(self):
        """测试配置单例模式"""
        from fundval.config import Config

        config1 = Config()
        config2 = Config()
        assert config1 is config2
