Fundval 发行包（默认 SQLite）

启动：
  Windows：双击 start.bat
  macOS/Linux：在终端执行 ./start.sh

默认数据库：
  - 若未设置 DATABASE_URL，后端会默认使用本地 SQLite：
    data/fundval.sqlite

切换到 Postgres：
  - 设置环境变量 DATABASE_URL，例如：
    postgresql://user:pass@127.0.0.1:5432/fundval

常用环境变量：
  - SECRET_KEY：建议生产环境设置高熵随机字符串
  - FUNDVAL_DATA_DIR：数据目录（默认由启动脚本设置为 ./data）
  - BACKEND_PORT / FRONTEND_PORT：端口（默认 8001 / 3000）

