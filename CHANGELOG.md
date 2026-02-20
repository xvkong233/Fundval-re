# Changelog

本项目的所有重要改动都会记录在本文件中。

格式参考 Keep a Changelog（但内容以本项目实际为准）。

## [Unreleased]

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
