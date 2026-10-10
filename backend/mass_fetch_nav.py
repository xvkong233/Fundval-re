#!/usr/bin/env python3
"""全市场基金历史净值批量下载。

用法: python3 mass_fetch_nav.py [--limit N] [--workers W]
- 从 all_fund_codes.json 读取全市场代码
- 优先下载股混类（混合/股票，排除债券/货币）
- 并发下载，限流保护，已下载跳过
- 输出: /home/hatch/workspace/fund_data/nav_<code>.json
"""
import asyncio
import aiohttp
import json
import os
import sys
import time
from pathlib import Path

DATA_DIR = Path("/home/hatch/workspace/fund_data")
CODES_FILE = DATA_DIR / "all_fund_codes.json"
API_URL = "https://fundmobapi.eastmoney.com/FundMNewApi/FundMNHisNetList"
PARAMS = {
    "IsShareNet": "true", "MobileKey": "1", "appType": "ttjj",
    "appVersion": "6.2.8", "cToken": "1", "deviceid": "1",
    "pageIndex": "1", "pageSize": "100000", "plat": "Iphone",
    "product": "EFund", "serverVersion": "6.2.8",
}

# 股混类关键词（优先）
EQUITY_KEYWORDS = ["混合型-偏股", "混合型-灵活", "股票型", "指数型-股票", "混合型-平衡"]

async def fetch_one(session, sem, code, ftype):
    path = DATA_DIR / f"nav_{code}.json"
    if path.exists() and path.stat().st_size > 100:
        return ("skip", code)
    async with sem:
        for attempt in range(3):
            try:
                params = dict(PARAMS)
                params["FCODE"] = code
                async with session.get(API_URL, params=params, timeout=aiohttp.ClientTimeout(total=30)) as r:
                    if r.status != 200:
                        await asyncio.sleep(2 ** attempt)
                        continue
                    data = await r.json()
                    # 限流检测
                    if not data.get("Success", True):
                        await asyncio.sleep(5 * (attempt + 1))  # 限流退避
                        continue
                    items = data.get("Datas", []) if isinstance(data, dict) else []
                    rows = []
                    for it in items:
                        d = it.get("FSRQ", "")
                        n = it.get("DWJZ", "")
                        if d and n:
                            rows.append({"date": d, "nav": str(n)})
                    if not rows:
                        return ("empty", code)
                    rows.sort(key=lambda x: x["date"])
                    path.write_text(json.dumps(rows))
                    return ("ok", code)
            except Exception:
                await asyncio.sleep(2 ** attempt)
            finally:
                await asyncio.sleep(1.0)  # 降速：每次请求间隔1秒
        return ("err", code)

async def main():
    limit = int(sys.argv[sys.argv.index("--limit")+1]) if "--limit" in sys.argv else 0
    workers = int(sys.argv[sys.argv.index("--workers")+1]) if "--workers" in sys.argv else 8

    codes = json.loads(CODES_FILE.read_text())
    # 读取类型信息用于优先级排序
    types = {}
    try:
        types = json.loads((DATA_DIR / "fund_types.json").read_text())
    except:
        pass

    # 优先级：股混类优先
    def prio(code):
        t = types.get(code, "")
        for i, kw in enumerate(EQUITY_KEYWORDS):
            if kw in t:
                return i
        return 99
    codes.sort(key=prio)
    if limit:
        codes = codes[:limit]

    print(f"待下载: {len(codes)} 只, 并发: {workers}", flush=True)
    sem = asyncio.Semaphore(workers)
    stats = {"ok": 0, "skip": 0, "http_fail": 0, "empty": 0, "err": 0}
    start = time.time()

    connector = aiohttp.TCPConnector(limit=workers*2)
    async with aiohttp.ClientSession(
        connector=connector,
        trust_env=True,  # 使用系统代理（https_proxy）
        headers={"User-Agent": "Mozilla/5.0", "Referer": "https://fund.eastmoney.com/"}
    ) as session:
        # 分批处理，每批500
        for bi in range(0, len(codes), 500):
            batch = codes[bi:bi+500]
            results = await asyncio.gather(*[fetch_one(session, sem, c, types.get(c,"")) for c in batch])
            for st, _ in results:
                stats[st] = stats.get(st, 0) + 1
            el = time.time() - start
            done = sum(stats.values())
            print(f"[{done}/{len(codes)}] ok={stats['ok']} skip={stats['skip']} fail={stats['http_fail']+stats['empty']+stats['err']} {el:.0f}s", flush=True)

    print(f"完成: {stats}, 用时 {time.time()-start:.0f}s")

if __name__ == "__main__":
    asyncio.run(main())
