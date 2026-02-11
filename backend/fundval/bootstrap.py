import secrets
import string
from .config import config


class BootstrapManager:
    """Bootstrap 密钥管理"""

    _bootstrap_key = None

    @classmethod
    def generate_key(cls):
        """生成高熵随机密钥"""
        if cls._bootstrap_key is None:
            # 生成 64 字符的随机字符串
            alphabet = string.ascii_letters + string.digits
            cls._bootstrap_key = ''.join(secrets.choice(alphabet) for _ in range(64))
        return cls._bootstrap_key

    @classmethod
    def get_key(cls):
        """获取 bootstrap_key"""
        if cls._bootstrap_key is None:
            cls.generate_key()
        return cls._bootstrap_key

    @classmethod
    def verify_key(cls, key):
        """验证 bootstrap_key"""
        if config.get('system_initialized'):
            return False
        return cls.get_key() == key

    @classmethod
    def invalidate_key(cls):
        """使密钥失效"""
        cls._bootstrap_key = None


def get_bootstrap_key():
    """获取 bootstrap_key（用于测试和启动）"""
    return BootstrapManager.get_key()


def verify_bootstrap_key(key):
    """验证 bootstrap_key"""
    return BootstrapManager.verify_key(key)


def invalidate_bootstrap_key():
    """使 bootstrap_key 失效"""
    BootstrapManager.invalidate_key()
