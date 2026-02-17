# Settings 用户信息与资产汇总

**Goal:** 在 `frontend-next` 的设置页展示当前登录用户信息（`/api/auth/me`）以及资产汇总（`/api/users/me/summary/`），并在前端侧对返回数据做归一化与安全计算。

**Scope:**
- 新增 `normalizeUserSummary()` 纯函数（支持字符串/数字输入，计算 `total_pnl_rate`，并对 0 成本做保护）
- 设置页启动时并发拉取 `me` + `summary`，失败时不阻断“修改密码”功能

**Files:**
- `frontend-next/src/lib/userSummary.ts`
- `frontend-next/src/lib/__tests__/userSummary.test.ts`
- `frontend-next/src/lib/api.ts`
- `frontend-next/src/app/settings/page.tsx`

