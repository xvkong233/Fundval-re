# Contract Success Branches 2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不依赖外网的前提下，补齐合同测试对“成功分支”的覆盖：`funds.estimate(success)` 与 `nav-history.sync(success)`，并保持 Python(golden) 与 Rust(candidate) 行为一致。

**Architecture:** 引入仅在合同测试环境启用的“种子数据源”（`contract-seed`），为 `estimate` 与 `nav_history` 提供确定性数据；同时通过环境变量让 `nav-history.sync` 在合同环境走 `contract-seed`，避免 EastMoney 外网请求导致不稳定。

**Tech Stack:** Django REST Framework、Axum、PostgreSQL、Node(合同测试)、Docker Compose。

---

### Task 1: 建 worktree + 复验基线全绿

**Files:**
- None

**Step 1: 创建 worktree（隔离开发）**

Run:
- `git worktree add .worktrees/contract-success-2 -b fix/contract-success-2`

Expected:
- 新目录 `.worktrees/contract-success-2` 存在
- 分支 `fix/contract-success-2` 指向当前 `main`

**Step 2: 复验合同测试基线**

Run:
- `cd .worktrees/contract-success-2; .\scripts\compose-contract.ps1 run -Build`

Expected:
- 所有 contract cases `PASS`

---

### Task 2: Python 增加 `contract-seed` 数据源（estimate + nav_history）

**Files:**
- Create: `backend/api/sources/contract_seed.py`
- Modify: `backend/api/sources/__init__.py`

**Step 1: 新增 `ContractSeedSource`（不联网、返回固定数据）**

Create `backend/api/sources/contract_seed.py`：

```python
import os
from datetime import datetime, date
from decimal import Decimal
from typing import Dict, Optional, List

from .base import BaseEstimateSource


class ContractSeedSource(BaseEstimateSource):
    def get_source_name(self) -> str:
        return "contract-seed"

    def fetch_estimate(self, fund_code: str) -> Optional[Dict]:
        # 合同测试只需要稳定 shape + 可解析字段，不依赖外部数据。
        return {
            "fund_code": fund_code,
            "fund_name": "Seed Fund (contract-tests)",
            "estimate_nav": Decimal("1.2345"),
            "estimate_growth": Decimal("0.12"),
            "estimate_time": datetime(2026, 2, 12, 14, 30),
        }

    def fetch_realtime_nav(self, fund_code: str) -> Optional[Dict]:
        return {
            "fund_code": fund_code,
            "nav": Decimal("1.2000"),
            "nav_date": date(2026, 2, 11),
        }

    def fetch_fund_list(self) -> list:
        return [{
            "fund_code": "000001",
            "fund_name": "Seed Fund (contract-tests)",
            "fund_type": "SEED",
        }]

    # eastmoney 扩展方法：给 nav_history.sync 用
    def fetch_nav_history(self, fund_code: str, start_date=None, end_date=None) -> List[Dict]:
        # 仅覆盖合同测试需要的区间：2026-02-12 ~ 2026-02-13
        items = [
            {"nav_date": date(2026, 2, 12), "unit_nav": Decimal("1.1300"), "accumulated_nav": Decimal("1.1300"), "daily_growth": Decimal("0.0100")},
            {"nav_date": date(2026, 2, 13), "unit_nav": Decimal("1.1400"), "accumulated_nav": Decimal("1.1400"), "daily_growth": Decimal("0.0088")},
        ]
        if start_date:
            items = [x for x in items if x["nav_date"] >= start_date]
        if end_date:
            items = [x for x in items if x["nav_date"] <= end_date]
        return items
```

**Step 2: 仅在合同环境注册 `contract-seed`**

Modify `backend/api/sources/__init__.py`：

```python
import os
from .contract_seed import ContractSeedSource

if os.getenv("ENABLE_CONTRACT_SEED_SOURCE") == "true":
    SourceRegistry.register(ContractSeedSource())
```

**Step 3: 验证 `GET /api/sources/` 包含 `contract-seed`（仅合同环境）**

Run:
- `.\scripts\compose-contract.ps1 run -Build`

Expected:
- `sources` 用例仍 PASS（golden/candidate 列表一致，且包含 `eastmoney`）

---

### Task 3: Rust 增加 `contract-seed` 数据源（estimate + nav_history）

**Files:**
- Create: `backend-rs/crates/api/src/contract_seed.rs`
- Modify: `backend-rs/crates/api/src/lib.rs`
- Modify: `backend-rs/crates/api/src/routes/sources.rs`
- Modify: `backend-rs/crates/api/src/routes/funds.rs`
- Modify: `backend-rs/crates/api/src/routes/nav_history.rs`

**Step 1: 新增模块 `contract_seed`（纯内存固定数据）**

Create `backend-rs/crates/api/src/contract_seed.rs`：

```rust
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;

pub struct SeedEstimate {
  pub fund_code: String,
  pub fund_name: String,
  pub estimate_nav: Decimal,
  pub estimate_growth: Decimal,
  pub estimate_time: NaiveDateTime,
}

pub fn fetch_estimate(fund_code: &str) -> SeedEstimate {
  SeedEstimate {
    fund_code: fund_code.to_string(),
    fund_name: "Seed Fund (contract-tests)".to_string(),
    estimate_nav: Decimal::from_str_exact("1.2345").unwrap(),
    estimate_growth: Decimal::from_str_exact("0.12").unwrap(),
    estimate_time: NaiveDate::from_ymd_opt(2026, 2, 12).unwrap().and_hms_opt(14, 30, 0).unwrap(),
  }
}

#[derive(Clone)]
pub struct SeedNav {
  pub nav_date: NaiveDate,
  pub unit_nav: Decimal,
  pub accumulated_nav: Option<Decimal>,
  pub daily_growth: Option<Decimal>,
}

pub fn fetch_nav_history(start: Option<NaiveDate>, end: Option<NaiveDate>) -> Vec<SeedNav> {
  let mut items = vec![
    SeedNav { nav_date: NaiveDate::from_ymd_opt(2026, 2, 12).unwrap(), unit_nav: Decimal::from_str_exact("1.1300").unwrap(), accumulated_nav: Some(Decimal::from_str_exact("1.1300").unwrap()), daily_growth: Some(Decimal::from_str_exact("0.0100").unwrap()) },
    SeedNav { nav_date: NaiveDate::from_ymd_opt(2026, 2, 13).unwrap(), unit_nav: Decimal::from_str_exact("1.1400").unwrap(), accumulated_nav: Some(Decimal::from_str_exact("1.1400").unwrap()), daily_growth: Some(Decimal::from_str_exact("0.0088").unwrap()) },
  ];
  if let Some(s) = start { items.retain(|x| x.nav_date >= s); }
  if let Some(e) = end { items.retain(|x| x.nav_date <= e); }
  items
}
```

**Step 2: 导出模块**

Modify `backend-rs/crates/api/src/lib.rs`：
- 增加 `pub mod contract_seed;`

**Step 3: `/api/sources/` 在合同环境追加 `contract-seed`**

Modify `backend-rs/crates/api/src/routes/sources.rs`：
- 保持默认只返回 `eastmoney`
- 当 `ENABLE_CONTRACT_SEED_SOURCE=true` 时：追加 `SourceItem { name: "contract-seed" }`

**Step 4: `funds.estimate` 支持 `source=contract-seed`**

Modify `backend-rs/crates/api/src/routes/funds.rs`：
- 当 `source=contract-seed`：直接返回 `contract_seed::fetch_estimate` 的 JSON（与 Python 对齐：`estimate_time` 用 `%Y-%m-%dT%H:%M:%S`）
- 其他 source：保持现状（`eastmoney`）

**Step 5: `nav-history.sync` 在合同环境走 `contract-seed`**

Modify `backend-rs/crates/api/src/routes/nav_history.rs`：
- 在 `sync_one()` 内选择数据来源：
  - 若 `NAV_HISTORY_SOURCE=contract-seed`：使用 `contract_seed::fetch_nav_history(effective_start, Some(effective_end))`
  - 否则：沿用 `eastmoney::fetch_nav_history(...)`

---

### Task 4: Python `nav_history.sync` 支持可配置数据源（默认 eastmoney）

**Files:**
- Modify: `backend/api/services/nav_history.py`

**Step 1: 通过 env 选择 nav_history 数据源**

Modify `backend/api/services/nav_history.py`：
- 将 `SourceRegistry.get_source('eastmoney')` 改为：
  - `source_name = os.getenv("NAV_HISTORY_SOURCE", "eastmoney")`
  - `source = SourceRegistry.get_source(source_name)`（不存在时抛清晰错误）

**Step 2: 合同环境下验证不走外网**

Run:
- `.\scripts\compose-contract.ps1 run -Build`

Expected:
- 所有 contract cases `PASS`

---

### Task 5: Docker Compose（contract）注入 env，开启种子数据源

**Files:**
- Modify: `docker-compose.contract.yml`

**Step 1: 为 golden/candidate 后端都设置 env**

Modify `docker-compose.contract.yml`：
- 在 `backend` 服务加：
  - `ENABLE_CONTRACT_SEED_SOURCE=true`
  - `NAV_HISTORY_SOURCE=contract-seed`
- 在 `backend-rs`（若需要）也加相同 env（可在该 override 文件里新增 `backend-rs:` 覆盖 environment）

**Step 2: 复验**

Run:
- `.\scripts\compose-contract.ps1 run -Build`

Expected:
- 全 PASS
- 脚本结束后 `docker ps -a | rg fundval-contract` 为空（不堆积容器）

---

### Task 6: 合同测试用例补齐：`funds.estimate(success)` + `nav-history.sync(success)`

**Files:**
- Modify: `tools/contract-tests/src/cases/funds.ts`
- Modify: `tools/contract-tests/src/cases/nav_history.ts`

**Step 1: 写 failing test：funds.estimate(success seed)**

Modify `tools/contract-tests/src/cases/funds.ts`：
- 新增：
  - `GET /api/funds/000001/estimate/?source=contract-seed`
  - 断言 status=200
  - 断言返回非 null
  - `assertSameSchema(golden.json, candidate.json, "$")`

**Step 2: 写 failing test：nav-history.sync(success seed)**

Modify `tools/contract-tests/src/cases/nav_history.ts`：
- 新增：
  - `POST /api/nav-history/sync/`，`fund_codes=["000001"]`，`start_date="2026-02-12"`，`end_date="2026-02-13"`
  - 断言 status=200
  - 断言 `json["000001"].success === true`
  - 断言 `json["000001"].count === 2`
  - `assertSameSchema(golden.json, candidate.json, "$")`

**Step 3: 跑合同测试确认从 RED→GREEN**

Run:
- `.\scripts\compose-contract.ps1 run -Build`

Expected:
- 全 PASS

---

### Task 7: 收尾（合并 + 清理）

**Files:**
- Modify/Add: 本计划涉及文件

**Step 1: 提交（小步提交，方便回滚）**

Run（示例）：
- `git add ...`
- `git commit -m "test(contract): add contract-seed source for offline success coverage"`

**Step 2: 合并回 main 并推送**

Run:
- `cd D:\zcode\FundVal-re; git merge --no-ff fix/contract-success-2`
- `git push origin main`

**Step 3: 清理 worktree**

Run:
- `git worktree remove .worktrees/contract-success-2 --force`
- `git branch -D fix/contract-success-2`

