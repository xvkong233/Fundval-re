# Changelog

本项目的所有重要改动都会记录在本文件中。

格式参考 Keep a Changelog（但内容以本项目实际为准）。

## [Unreleased]

### Added
- 预测模型训练改用闭式岭回归（`train_ols_closed_form`）：20 维特征直接求解，比 3-epoch SGD 更稳定、更快，且不受学习率/epoch 超参影响；`OlsModel` 结构不变，前后端兼容。
- 新增前端共享常量 `frontend/src/lib/forecast.ts`（训练参数 TS 镜像），与后端 `forecast` 模块常量保持同步。

### Changed
- 训练参数（`model_name`/`horizon`/`lag_k`）抽取为后端 `forecast` 模块共享常量，前后端统一引用，避免两处硬编码不一致导致分析任务查不到模型。
- 合并两份重复的预测模型训练逻辑：抽取 `build_forecast_training_dataset`（数据集构建）、`fit_forecast_model`（训练）、`upsert_forecast_model_row`（模型落库）为模块级公共函数，`exec_forecast_model_train` 与 `train_global_model` 均复用。
- 按 fund-analysis-v2 规划彻底替换旧版分析：删除 `GET /api/funds/{fund_code}/analytics` 路由及 `fund_analytics.rs`、相关路由测试和前端 `getFundAnalytics`（`analytics/` 纯函数模块及其单元测试保留）。

### Fixed
- 基金分析 v2 任务不再强依赖 quant-service：`metrics`/`macd`/`grid`/`scheduled` 四处调用失败时降级为 `{"error": ...}`（与 `ts` 调用一致），quant-service 未启动时预测链路仍可正常产出 forecast，快照可落库。
- 修复集成测试偶发死锁：路由入队后 `tokio::spawn` 后台 worker 的 `if !cfg!(test)` 守卫在集成测试中失效（lib 并非以 test cfg 编译），导致测试内多个 worker 并发抢单连接 SQLite 池。改为 `AppState::spawn_background_workers` 显式开关（默认 true，生产行为不变），9 处路由改用该开关，集成测试统一关闭。

## [1.4.0] - 2026-02-21

### Added
- 新增“关联板块同类”信号缓存：`ml_sector_model`（板块模型）与 `fund_signal_snapshot`（基金信号快照）。
- 新增基金信号接口 `GET /api/funds/{fund_code}/signals`：输出位置分桶（20/60/20）与两套窗口（5T/20T）的抄底/反转概率。
- 基金详情页新增“预测信号（ML）”区块：同屏展示 5T+20T，并支持查看关联板块列表。
- 嗅探页“购买建议”升级：叠加 ML 信号（位置/抄底/反转）标签，并提升表格可读性（sticky + 固定高度）。

## [1.3.0] - 2026-02-20

### Added
- 新增“净值缓存爬虫（管理员）”配置页：可视化调整批次、间隔、每日上限、抖动、数据源回退等防封锁参数。
- 后端新增管理员接口 `GET/PUT /api/admin/crawl/config`，用于读取/写入爬虫配置。
- 后端爬虫新增防封锁能力：每日执行上限、稳定抖动（按基金代码打散节奏）、多数据源 fallback。

### Fixed
- GitHub Actions `release-tag.yml` 兼容性修复（`"on"` 键避免 YAML 1.1 误解析）。

## [1.2.1] - 2026-02-20

### Added
- 基金详情页新增“同类分位综合分（value_score）”“经济学确定性等价（CE，γ 可调）”“短线策略（趋势优先）”。
- 基金专业指标接口支持 `gamma` 参数，并返回 `value_score` / `ce` / `short_term`。

## [1.2.0] - 2026-02-20

### Added
- 支持 SQLite / Postgres 双后端：发行版默认 SQLite，Docker/生产可配置 Postgres。
- 新增跨平台发布打包产物（模板脚本、Windows 安装器配置等）。
- 新增 SQLite migrations 与 smoke tests，CI 更早发现兼容性问题。

### Changed
- 后端数据库接入调整为 `sqlx::AnyPool`，migrations 按 `postgres/`、`sqlite/` 分目录管理。
- CI 改为仅在推送版本 Tag（`v*`）时触发发布流程。

### Fixed
- 修复 GitHub Actions workflow YAML 语法错误导致无法运行的问题。

## [1.1.0] - 2026-02-19

### Added
- 新增基金嗅探功能：定期从 DeepQ 星标页同步基金到自选的独立分组。
- 新增嗅探页，并支持排序/筛选（标签筛选为“同时包含所选标签”）。
- 基金/自选/嗅探页面新增“关联板块”展示。
- 新增数据源 `tushare`（Token 在设置页配置）。

### Changed
- 基金详情页标题区展示优化：基金名称与类型同一行显示，长标题自动省略避免溢出。

### Fixed
- 修复天天基金数据源页解析报错（`EOF while parsing a string`）。
