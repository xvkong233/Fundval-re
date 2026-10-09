use serde_json::json;
use sqlx::Row;

use super::dataset::{DatasetConfig, build_trigger_samples_for_peer};
use super::features::feature_names;
use super::logreg::{LogRegModel, LogRegTrainConfig, train_logreg};

pub const PEER_CODE_ALL: &str = "__all__";

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

/// 留出法评估：按样本顺序取后 `holdout_ratio` 作验证集（样本按时间生成），
/// 返回准确率/精确率/召回率/F1/AUC。样本过少时返回 None。
fn evaluate_holdout(
    model: &LogRegModel,
    x: &[Vec<f64>],
    y: &[f64],
    holdout_ratio: f64,
) -> Option<serde_json::Value> {
    let n = x.len();
    if n < 20 {
        return None;
    }
    let cut = ((n as f64) * (1.0 - holdout_ratio)) as usize;
    if cut < 10 || cut >= n {
        return None;
    }
    let (mut tp, mut tn, mut fp, mut fn_) = (0_i64, 0_i64, 0_i64, 0_i64);
    let mut scored: Vec<(f64, f64)> = Vec::with_capacity(n - cut);
    for i in cut..n {
        let p = model.predict_proba(&x[i]).unwrap_or(0.5);
        let actual = y[i] >= 0.5;
        scored.push((p, y[i]));
        match (p >= 0.5, actual) {
            (true, true) => tp += 1,
            (false, false) => tn += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
        }
    }
    let total = (tp + tn + fp + fn_) as f64;
    let acc = (tp + tn) as f64 / total;
    let precision = tp as f64 / (tp + fp).max(1) as f64;
    let recall = tp as f64 / (tp + fn_).max(1) as f64;
    let f1 = if precision + recall > 1e-12 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    // Mann-Whitney AUC
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let n_pos = scored.iter().filter(|(_, y)| *y >= 0.5).count() as f64;
    let n_neg = scored.len() as f64 - n_pos;
    let auc = if n_pos > 0.0 && n_neg > 0.0 {
        let mut tp_c = 0.0_f64;
        let mut auc_acc = 0.0_f64;
        let mut i = 0;
        while i < scored.len() {
            let mut j = i + 1;
            while j < scored.len() && (scored[j].0 - scored[i].0).abs() < 1e-12 {
                j += 1;
            }
            let (mut p_add, mut n_add) = (0.0_f64, 0.0_f64);
            for &(_, yy) in &scored[i..j] {
                if yy >= 0.5 {
                    p_add += 1.0;
                } else {
                    n_add += 1.0;
                }
            }
            auc_acc += n_add * tp_c + n_add * p_add / 2.0;
            tp_c += p_add;
            i = j;
        }
        auc_acc / (n_pos * n_neg)
    } else {
        0.5
    };
    Some(json!({
        "n": scored.len(),
        "accuracy": acc,
        "precision": precision,
        "recall": recall,
        "f1": f1,
        "auc": auc,
    }))
}

pub async fn train_and_store_sector_model(    pool: &sqlx::AnyPool,
    peer_code: &str,
    source_name: &str,
    task: MlTask,
    cfg: &DatasetConfig,
) -> Result<(), String> {
    let samples = if peer_code.trim() == PEER_CODE_ALL {
        super::dataset::build_trigger_samples_for_all_funds(pool, source_name, cfg).await?
    } else {
        build_trigger_samples_for_peer(pool, peer_code, source_name, cfg).await?
    };
    if samples.is_empty() {
        return Ok(());
    }

    let feature_names = feature_names();

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

    // 类别平衡权重：少数类样本加权，避免模型退化为恒预测多数类
    let n_pos = y.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = y.len() as f64 - n_pos;
    let pos_weight = if n_pos > 0.0 { n_neg / n_pos } else { 1.0 };

    let train_cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400, // 调优最优值（200基金验证）
        l2: 0.05,    // 调优最优值（200基金验证）
        pos_weight,
    };
    let model = train_logreg(&x, &y, &train_cfg).ok_or("train_logreg failed")?;

    let positives = n_pos as i64;
    let total = y.len() as i64;
    // 留出法快速评估（按时间后 20% 作验证集），写入 metrics 供前端展示
    let eval = evaluate_holdout(&model, &x, &y, 0.2);
    let metrics = json!({
        "sample_size": total,
        "positive": positives,
        "positive_rate": if total > 0 { (positives as f64) / (total as f64) } else { 0.0 },
        "holdout": eval,
        "train": {
            "learning_rate": train_cfg.learning_rate,
            "epochs": train_cfg.epochs,
            "l2": train_cfg.l2,
            "pos_weight": train_cfg.pos_weight,
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
