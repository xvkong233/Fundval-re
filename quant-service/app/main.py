from fastapi import FastAPI

from app.routes.fund_strategies_compare import router as fund_strategies_compare_router
from app.routes.fund_strategies_ts import router as fund_strategies_ts_router
from app.routes.macd import router as macd_router
from app.routes.pytrader_backtest import router as pytrader_router
from app.routes.xalpha_backtest import router as xalpha_backtest_router
from app.routes.xalpha_like import router as xalpha_like_router
from app.settings import settings

app = FastAPI(title=settings.service_name)
app.include_router(macd_router)
app.include_router(fund_strategies_ts_router)
app.include_router(fund_strategies_compare_router)
app.include_router(xalpha_like_router)
app.include_router(xalpha_backtest_router)
app.include_router(pytrader_router)


@app.get("/health")
def health():
    return {"ok": True, "service": settings.service_name}
