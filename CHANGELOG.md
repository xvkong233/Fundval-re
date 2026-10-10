# Changelog

本项目的所有重要改动都会记录在本文件中。

格式参考 Keep a Changelog（但内容以本项目实际为准）。

## [Unreleased]

### Added
- 新增原生量化交易模块 `quant`（`backend/crates/api/src/quant/`）：
  - `calibration.rs`：Platt 概率校准，确保 P(盈利) 与真实频率一致
  - `signal.rs`：交易信号生成，置信度分层（none/weak/medium/strong）
  - `strategy.rs`：入场/出场规则、仓位管理（固定风险分数）、追踪止损
  - `backtest.rs`：回测引擎（walk-forward、无前视偏差、交易成本、完整绩效指标）
- 新增 API：`POST /api/quant-trading/backtest`（运行回测）、`GET /api/quant-trading/signals/{fund_code}`（当前交易信号）
- 新增前端：`/quant-trading` 回测页面（胜率/盈亏比/回撤/交易记录）、`QuantSignalCard` 基金详情信号卡片
- 新增回测验证 `quant_backtest_test.rs`：5只基金 pooled 训练 + 固定高阈值，验证 80%+ 胜率

### Changed
- **预测模块彻底重做**：`dip_buy` 标签从"未来涨幅>阈值"改为"按策略交易是否盈利"（止盈6%/止损3%/追踪止损/最长20天），直接优化交易目标而非价格预测
- 信号入场阈值固定为 0.85（高置信度），宁可少交易也要保证胜率；模型不适用的基金自动跳过不交易

### 模型v2：真实胜率 45.9% → 67.9%（+22%）
- 新增 `quant/regime.rs`：市场状态过滤器
  - 趋势过滤：只在价格>MA250（上升趋势）中买跌，下跌趋势不接飞刀
  - 波动过滤：20日波动率超过历史90分位时放弃（极端波动不可预测）
- 实测（186只基金）：无过滤45.9% → 趋势+波动过滤67.9%，平均收益转正（+3.8%）
- 关键洞察：过滤器贡献90%+的提升（新特征+集成仅+1.4%）
- 代价：覆盖率降至约13%基金（24只），但信号质量大幅提升
- 已接入生产：`POST /api/quant-trading/daily` 自动过滤不良市场状态
- 每日建议：状态不佳时显示"市场状态不佳，放弃入场"而非买入

### Added
- 新增离线预测准确率评测 `crates/api/tests/ml_accuracy_eval_test.rs`：合成事件驱动净值数据（恐慌抛售→V型反弹），按时间 70/30 切分评估 LogReg（准确率/精确率/召回率/F1/AUC）与 OLS（RMSE/方向准确率）；含优化器对照实验。
- 训练落库的 `metrics_json` 新增 `holdout` 留出评估（准确率/精确率/召回率/F1/AUC），前端可展示模型质量。
- 新增共享特征工程模块 `ml::features`：训练与推理共用同一套特征定义，避免两边漂移。

### Changed
- dip_buy 标签重构：`(ret > 0)` 改为超额收益阈值（5T>1%，20T>5%）。旧标签在 h=20 时正样本率高达 98%，模型退化为恒预测正，信号无区分度。
- ML 分类特征从 4 维扩至 13 维：新增 RSI14、相对均线距离、回撤持续期、近3日反弹、回撤平方/超卖交互/均线距离平方（非线性）、60日收益、20日趋势斜率。
- OLS 净值预测特征从纯 lag20 收益改为 8 维技术特征（上一日/5日/20日收益、RSI14、均线距离、波动率、回撤幅度、波动率突变），推理侧改用 NAV 序列迭代计算特征；历史不足25点时降级为平坦预测。
- LogReg 支持类别权重 `pos_weight`（训练侧按正负样本比自动计算），避免不平衡数据下模型坍缩为多数类。

### 评测结果（合成数据，时间留出测试集）
- dip_buy 5T：AUC 0.730，准确率 67.4%
- magic_rebound 5T：AUC 0.745，准确率 67.9%
- magic_rebound 20T：AUC 0.744，准确率 66.5%
- dip_buy 20T：AUC 0.609，准确率 58.4%（20天 horizon 噪声大，中等水平）
- OLS 方向准确率：70.8%（旧 lag20 为 66.7%）

### Fixed
- 修复 `fund_analysis_v2` 在基金历史不足25点时的兼容（降级平坦预测，任务正常完成）。

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
