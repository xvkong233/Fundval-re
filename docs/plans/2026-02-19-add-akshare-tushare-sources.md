# 增加数据源：AkShare / Tushare

日期：2026-02-19

## 目标

在现有 `tiantian/danjuan/ths` 的基础上新增两个数据源：

- `tushare`：提供基金净值与历史净值同步能力，并支持在“设置”页面配置 Token

这些数据源需要能被以下能力复用：

- 基金列表/详情页的数据源下拉选择（前端透传 `source`）
- `batch_update_nav`（刷新基金最新净值）
- `nav-history/sync`（同步历史净值）
- `/sources` 运维页的健康度探测（`/api/sources/health/`）
- 估值准确率计算（`/api/sources/{source}/accuracy/calculate`）的“实际净值”抓取

## 方案（实现约束）

### AkShare

已移除（不再内置该数据源）。

### Tushare

使用官方接口：

- `POST https://api.tushare.pro`
- `api_name=fund_nav`
- Token 存储在后端 `config.json`（Docker 场景对应 `/app/config/config.json` 的 volume）

Token 配置方式：

- 后端提供受管理员（`is_staff`）保护的接口：
  - `GET /api/settings/tushare_token/`：查询是否已配置（仅返回 hint，不返回明文）
  - `PUT /api/settings/tushare_token/`：写入/清空 token
- 前端设置页提供 UI 写入 token

## 兼容性与注意事项

- `tushare` 数据一般是日频净值，不保证“盘中实时”；系统会以其返回的最新日期为准。
- 若未配置 Token，`tushare`：
  - 在健康度探测与同步/刷新时会返回明确错误提示
  - 其他数据源不受影响
