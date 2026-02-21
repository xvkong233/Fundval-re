use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct LogRegTrainConfig {
    pub learning_rate: f64,
    pub epochs: usize,
    pub l2: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRegModel {
    pub weights: Vec<f64>,
    pub bias: f64,
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
}

impl LogRegModel {
    pub fn predict_proba(&self, x: &[f64]) -> Option<f64> {
        if x.len() != self.weights.len() || x.len() != self.mean.len() || x.len() != self.std.len()
        {
            return None;
        }

        let mut z = self.bias;
        for (xi, ((mean_i, std_i), w_i)) in x.iter().zip(
            self.mean
                .iter()
                .zip(self.std.iter())
                .zip(self.weights.iter()),
        ) {
            let s = if std_i.abs() < 1e-12 {
                xi - mean_i
            } else {
                (xi - mean_i) / std_i
            };
            z += w_i * s;
        }
        Some(sigmoid(z))
    }
}

pub fn train_logreg(x: &[Vec<f64>], y: &[f64], cfg: &LogRegTrainConfig) -> Option<LogRegModel> {
    if x.is_empty() || x.len() != y.len() {
        return None;
    }
    let n = x.len();
    let d = x[0].len();
    if d == 0 {
        return None;
    }
    if x.iter().any(|row| row.len() != d) {
        return None;
    }

    let mut mean = vec![0.0_f64; d];
    for row in x {
        for (m, &v) in mean.iter_mut().zip(row.iter()) {
            *m += v;
        }
    }
    for m in &mut mean {
        *m /= n as f64;
    }

    let mut var = vec![0.0_f64; d];
    for row in x {
        for (acc, (&xj, &mj)) in var.iter_mut().zip(row.iter().zip(mean.iter())) {
            let v = xj - mj;
            *acc += v * v;
        }
    }
    for acc in &mut var {
        *acc /= n as f64;
    }
    let mut std = var.into_iter().map(|v| v.sqrt()).collect::<Vec<_>>();
    for s in &mut std {
        if s.abs() < 1e-12 {
            *s = 1.0;
        }
    }

    let mut w = vec![0.0_f64; d];
    let mut b = 0.0_f64;

    let lr = cfg.learning_rate.clamp(1e-6, 10.0);
    let l2 = cfg.l2.max(0.0);

    for _ in 0..cfg.epochs.max(1) {
        let mut grad_w = vec![0.0_f64; d];
        let mut grad_b = 0.0_f64;

        for (row, &yy) in x.iter().zip(y.iter()) {
            let yy = if yy >= 0.5 { 1.0 } else { 0.0 };

            let mut z = b;
            let mut xs = vec![0.0_f64; d];
            for j in 0..d {
                let s = (row[j] - mean[j]) / std[j];
                xs[j] = s;
                z += w[j] * s;
            }
            let p = sigmoid(z);
            let err = p - yy;

            for j in 0..d {
                grad_w[j] += err * xs[j];
            }
            grad_b += err;
        }

        let inv_n = 1.0 / (n as f64);
        for j in 0..d {
            grad_w[j] = grad_w[j] * inv_n + l2 * w[j];
        }
        grad_b *= inv_n;

        for j in 0..d {
            w[j] -= lr * grad_w[j];
        }
        b -= lr * grad_b;
    }

    Some(LogRegModel {
        weights: w,
        bias: b,
        mean,
        std,
    })
}

fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 {
        let ez = (-z).exp();
        1.0 / (1.0 + ez)
    } else {
        let ez = z.exp();
        ez / (1.0 + ez)
    }
}
