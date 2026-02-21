use serde_json::json;
use sqlx::Row;

use super::dataset::{DatasetConfig, build_trigger_samples_for_peer};
use super::logreg::{LogRegModel, LogRegTrainConfig, train_logreg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MlTask {
    DipBuy,
    MagicRebound,
}

impl MlTask {
    pub fn as_str(&self) -> &'static str {
        match self {
            MlTask::DipBuy => "dip_buy",
            MlTask::MagicRebound => "magic_rebound",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SectorModelRecord {
    pub peer_code: String,
    pub task: MlTask,
    pub horizon_days: i64,
    pub feature_names: Vec<String>,
    pub model: LogRegModel,
    pub metrics: serde_json::Value,
}

pub async fn train_and_store_sector_model(
    pool: &sqlx::AnyPool,
    peer_code: &str,
    source_name: &str,
    task: MlTask,
    cfg: &DatasetConfig,
) -> Result<(), String> {
    let samples = build_trigger_samples_for_peer(pool, peer_code, source_name, cfg).await?;
    if samples.is_empty() {
        return Ok(());
    }

    let feature_names = vec![
        "dd_mag".to_string(),
        "ret5".to_string(),
        "ret20".to_string(),
        "vol20".to_string(),
    ];

    let mut x: Vec<Vec<f64>> = Vec::with_capacity(samples.len());
    let mut y: Vec<f64> = Vec::with_capacity(samples.len());
    for s in &samples {
        x.push(s.features.clone());
        let label = match task {
            MlTask::DipBuy => s.dip_buy_success,
            MlTask::MagicRebound => s.magic_rebound,
        };
        y.push(if label { 1.0 } else { 0.0 });
    }

    let train_cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 600,
        l2: 0.1,
    };
    let model = train_logreg(&x, &y, &train_cfg).ok_or("train_logreg failed")?;

    let positives = y.iter().filter(|v| **v >= 0.5).count() as i64;
    let total = y.len() as i64;
    let metrics = json!({
        "sample_size": total,
        "positive": positives,
        "positive_rate": if total > 0 { (positives as f64) / (total as f64) } else { 0.0 },
        "train": {
            "learning_rate": train_cfg.learning_rate,
            "epochs": train_cfg.epochs,
            "l2": train_cfg.l2,
        }
    });

    let feature_names_json = serde_json::to_string(&feature_names).map_err(|e| e.to_string())?;
    let model_json = serde_json::to_string(&model).map_err(|e| e.to_string())?;
    let metrics_json = serde_json::to_string(&metrics).map_err(|e| e.to_string())?;

    sqlx::query(
        r#"
        INSERT INTO ml_sector_model (
          peer_code, task, horizon_days,
          feature_names_json, model_json, metrics_json,
          trained_at, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (peer_code, task, horizon_days) DO UPDATE SET
          feature_names_json = excluded.feature_names_json,
          model_json = excluded.model_json,
          metrics_json = excluded.metrics_json,
          trained_at = CURRENT_TIMESTAMP,
          updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(peer_code)
    .bind(task.as_str())
    .bind(cfg.horizon_days as i64)
    .bind(feature_names_json)
    .bind(model_json)
    .bind(metrics_json)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn get_sector_model(
    pool: &sqlx::AnyPool,
    peer_code: &str,
    task: MlTask,
    horizon_days: i64,
) -> Result<Option<SectorModelRecord>, String> {
    let row = sqlx::query(
        r#"
        SELECT
          peer_code,
          task,
          horizon_days,
          feature_names_json,
          model_json,
          metrics_json
        FROM ml_sector_model
        WHERE peer_code = $1 AND task = $2 AND horizon_days = $3
        "#,
    )
    .bind(peer_code)
    .bind(task.as_str())
    .bind(horizon_days)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(row) = row else {
        return Ok(None);
    };

    let feature_names_json: String = row.get("feature_names_json");
    let model_json: String = row.get("model_json");
    let metrics_json: String = row.get("metrics_json");

    let feature_names: Vec<String> =
        serde_json::from_str(&feature_names_json).map_err(|e| e.to_string())?;
    let model: LogRegModel = serde_json::from_str(&model_json).map_err(|e| e.to_string())?;
    let metrics: serde_json::Value =
        serde_json::from_str(&metrics_json).map_err(|e| e.to_string())?;

    Ok(Some(SectorModelRecord {
        peer_code: peer_code.to_string(),
        task,
        horizon_days,
        feature_names,
        model,
        metrics,
    }))
}
