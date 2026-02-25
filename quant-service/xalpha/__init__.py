"""
本目录为 Qbot 上游仓库携带的 xalpha 源码拷贝。

注意：原版 xalpha 默认会在导入时拉起大量可视化/抓取相关依赖（pyecharts/scipy/bs4/sqlalchemy 等）。
本项目为了在 docker/CI 环境中更稳定可用，改为 **最小化 __init__ 副作用**：

- `import xalpha` 不会强制导入全部子模块
- 需要使用具体能力时，请显式 `import xalpha.<module>`

这不影响本项目的 `quant-service/app/qbot_xalpha/*`（本项目自研的“显式输入序列 + 显式日历”策略实现）。
"""

__version__ = "0.11.7"
__author__ = "refraction-ray"

__all__ = ["__version__", "__author__"]
