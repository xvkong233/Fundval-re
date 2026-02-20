# Changelog

本项目的所有重要改动都会记录在本文件中。

格式参考 Keep a Changelog（但内容以本项目实际为准）。

## [Unreleased]

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
