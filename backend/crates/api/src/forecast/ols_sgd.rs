use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OlsModel {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub residual_sigma: f64,
}

/// 训练超参（历史 SGD 版本遗留，当前训练统一使用闭式岭回归 `train_ols_closed_form`）。
#[derive(Debug, Clone, Copy)]
pub struct OlsTrainConfig {
    pub learning_rate: f64,
    pub epochs: usize,
    pub l2: f64,
}

fn mean_std(x: &[Vec<f64>]) -> Option<(Vec<f64>, Vec<f64>)> {
    if x.is_empty() {
        return None;
    }
    let d = x[0].len();
    if d == 0 || x.iter().any(|r| r.len() != d) {
        return None;
    }

    let n = x.len() as f64;
    let mut mean = vec![0.0_f64; d];
    for row in x {
        for (m, &v) in mean.iter_mut().zip(row.iter()) {
            *m += v;
        }
    }
    for m in &mut mean {
        *m /= n;
    }

    let mut var = vec![0.0_f64; d];
    for row in x {
        for (acc, (&v, &m)) in var.iter_mut().zip(row.iter().zip(mean.iter())) {
            let dv = v - m;
            *acc += dv * dv;
        }
    }
    for acc in &mut var {
        *acc /= n.max(1.0);
    }
    let mut std = var.into_iter().map(|v| v.sqrt()).collect::<Vec<_>>();
    for s in &mut std {
        if s.abs() < 1e-12 {
            *s = 1.0;
        }
    }

    Some((mean, std))
}

pub fn train_ols_sgd(x: &[Vec<f64>], y: &[f64], cfg: &OlsTrainConfig) -> Option<OlsModel> {
    if x.is_empty() || x.len() != y.len() {
        return None;
    }
    let d = x[0].len();
    if d == 0 || x.iter().any(|r| r.len() != d) {
        return None;
    }

    let (mean, std) = mean_std(x)?;

    let mut w = vec![0.0_f64; d];
    let mut b = 0.0_f64;

    let lr = cfg.learning_rate.clamp(1e-6, 10.0);
    let l2 = cfg.l2.max(0.0);
    let epochs = cfg.epochs.max(1).min(5000);

    for _ in 0..epochs {
        for (row, &yy) in x.iter().zip(y.iter()) {
            let mut pred = b;
            for j in 0..d {
                let xs = (row[j] - mean[j]) / std[j];
                pred += w[j] * xs;
            }
            let err = pred - yy;
            for j in 0..d {
                let xs = (row[j] - mean[j]) / std[j];
                let grad = err * xs + l2 * w[j];
                w[j] -= lr * grad;
            }
            b -= lr * err;
        }
    }

    // residual sigma (RMSE)
    let mut sse = 0.0_f64;
    for (row, &yy) in x.iter().zip(y.iter()) {
        let mut pred = b;
        for j in 0..d {
            let xs = (row[j] - mean[j]) / std[j];
            pred += w[j] * xs;
        }
        let err = pred - yy;
        sse += err * err;
    }
    let rmse = (sse / (x.len() as f64).max(1.0)).sqrt();

    Some(OlsModel {
        weights: w,
        bias: b,
        mean,
        std,
        residual_sigma: rmse.max(0.0),
    })
}

impl OlsModel {
    pub fn predict(&self, x: &[f64]) -> Option<f64> {
        if x.len() != self.weights.len() || x.len() != self.mean.len() || x.len() != self.std.len() {
            return None;
        }
        let mut pred = self.bias;
        for j in 0..x.len() {
            let xs = (x[j] - self.mean[j]) / self.std[j];
            pred += self.weights[j] * xs;
        }
        Some(pred)
    }
}

/// 闭式岭回归：w = (Xs'Xs + λI)^{-1} Xs'yc，bias = mean(y)。
///
/// 与 `train_ols_sgd` 使用相同的特征标准化，返回的 `OlsModel` 结构完全兼容，
/// 可直接替换 SGD 版本。特征维度很小（lag_k=20），直接求解比 SGD 更稳定、
/// 更快，且不受学习率 / epoch 数等超参影响（SGD 仅 3 个 epoch 时权重几乎不动，
/// 预测退化为历史平均漂移）。
///
/// 使用 Gauss-Jordan 消元（部分主元）求解；若某主元过小（零方差特征），
/// 对应权重保持为 0，避免奇异矩阵导致失败。
pub fn train_ols_closed_form(x: &[Vec<f64>], y: &[f64], l2: f64) -> Option<OlsModel> {
    if x.is_empty() || x.len() != y.len() {
        return None;
    }
    let d = x[0].len();
    if d == 0 || x.iter().any(|r| r.len() != d) {
        return None;
    }

    let (mean, std) = mean_std(x)?;
    let n = x.len();
    let lambda = l2.max(1e-12);

    let y_mean: f64 = y.iter().sum::<f64>() / n as f64;

    // A = Xs'Xs + λI (d×d), b = Xs'yc (d)
    let mut a = vec![vec![0.0_f64; d]; d];
    let mut b = vec![0.0_f64; d];
    for (row, &yy) in x.iter().zip(y.iter()) {
        let yc = yy - y_mean;
        // 先算标准化特征，避免重复计算
        for j in 0..d {
            let xsj = (row[j] - mean[j]) / std[j];
            b[j] += xsj * yc;
            for k in j..d {
                let xsk = (row[k] - mean[k]) / std[k];
                a[j][k] += xsj * xsk;
            }
        }
    }
    for j in 0..d {
        for k in 0..j {
            a[j][k] = a[k][j];
        }
        a[j][j] += lambda;
    }

    // Gauss-Jordan 消元（部分主元）解 A w = b
    let mut w = vec![0.0_f64; d];
    let mut aug: Vec<Vec<f64>> = a
        .into_iter()
        .zip(b.into_iter())
        .map(|(mut row, bi)| {
            row.push(bi);
            row
        })
        .collect();
    for col in 0..d {
        // 选主元
        let mut pivot = col;
        let mut pivot_abs = aug[col][col].abs();
        for r in (col + 1)..d {
            let v = aug[r][col].abs();
            if v > pivot_abs {
                pivot_abs = v;
                pivot = r;
            }
        }
        if pivot_abs < 1e-12 {
            // 奇异方向（零方差特征）：权重保持 0
            continue;
        }
        aug.swap(col, pivot);
        let diag = aug[col][col];
        for k in col..=d {
            aug[col][k] /= diag;
        }
        for r in 0..d {
            if r != col {
                let factor = aug[r][col];
                if factor != 0.0 {
                    for k in col..=d {
                        aug[r][k] -= factor * aug[col][k];
                    }
                }
            }
        }
    }
    for j in 0..d {
        w[j] = aug[j][d];
        if !w[j].is_finite() {
            w[j] = 0.0;
        }
    }

    // residual sigma (RMSE)
    let model = OlsModel {
        weights: w,
        bias: y_mean,
        mean,
        std,
        residual_sigma: 0.0,
    };
    let mut sse = 0.0_f64;
    for (row, &yy) in x.iter().zip(y.iter()) {
        let pred = model.predict(row).unwrap_or(y_mean);
        let err = pred - yy;
        sse += err * err;
    }
    let rmse = (sse / n as f64).sqrt();

    Some(OlsModel {
        residual_sigma: rmse.max(0.0),
        ..model
    })
}
