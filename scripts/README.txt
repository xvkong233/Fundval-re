Scripts
=======

Docker（Windows PowerShell 推荐）
-------------------------------

默认策略：每次启动前先删除旧容器（不删除 volume），避免堆积容器/网络。

  # 启动（会先 down --remove-orphans，再 up -d --no-build）
  .\scripts\compose-dev.ps1 up

  # 启动并重新构建镜像
  .\scripts\compose-dev.ps1 up -Build

  # 查看状态
  .\scripts\compose-dev.ps1 status

  # 跟随日志
  .\scripts\compose-dev.ps1 logs

  # 停止并删除容器/网络（保留 volume 数据）
  .\scripts\compose-dev.ps1 down

常用覆盖：

  .\scripts\compose-dev.ps1 up -ProjectName fundval-dev -FrontendNextPort 19700 -BackendRsPort 19701

