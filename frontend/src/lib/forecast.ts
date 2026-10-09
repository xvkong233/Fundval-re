/**
 * 全市场预测模型的共享参数（TS 镜像）。
 *
 * 必须与后端 `backend/crates/api/src/forecast/mod.rs` 中的
 * FORECAST_MODEL_NAME / FORECAST_HORIZON / FORECAST_LAG_K 保持一致：
 * 后端按 (model_name, source, horizon, lag_k) 精确匹配模型行，
 * 参数不一致会导致分析任务查不到已训练的模型。
 */
export const FORECAST_MODEL_NAME = "global_ols_v1";
export const FORECAST_HORIZON = 60;
export const FORECAST_LAG_K = 20;
export const FORECAST_SOURCE = "tiantian";

export interface TrainForecastModelPayload {
  source?: string;
  model_name?: string;
  horizon?: number;
  lag_k?: number;
}

/** 训练全市场预测模型的默认请求参数（与后端共享常量对齐）。 */
export function defaultTrainForecastModelPayload(): Required<TrainForecastModelPayload> {
  return {
    source: FORECAST_SOURCE,
    model_name: FORECAST_MODEL_NAME,
    horizon: FORECAST_HORIZON,
    lag_k: FORECAST_LAG_K,
  };
}
