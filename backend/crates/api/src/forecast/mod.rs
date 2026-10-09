pub mod ols_sgd;

/// 全市场预测模型的共享参数。
///
/// 训练任务（`exec_forecast_model_train`）与分析任务（`exec_fund_analysis_v2_compute`）
/// 必须使用同一组参数：`load_forecast_model` 按 `(model_name, source, horizon, lag_k)`
/// 精确匹配模型行，任何一处私自改动都会导致分析任务查不到模型（进而静默触发重训覆盖）。
/// 前端 `frontend/src/lib/forecast.ts` 维护着同一组常量的 TS 镜像，改动时请同步。
pub const FORECAST_MODEL_NAME: &str = "global_ols_v1";
pub const FORECAST_HORIZON: i64 = 60;
pub const FORECAST_LAG_K: i64 = 20;
