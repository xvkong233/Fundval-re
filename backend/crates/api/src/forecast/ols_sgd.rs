use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OlsModel {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub residual_sigma: f64,
}

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

