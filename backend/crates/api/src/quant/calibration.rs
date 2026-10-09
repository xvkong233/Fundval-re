//! 概率校准：确保模型输出的 P(盈利) 与真实频率一致。
//!
//! 80%+ 胜率的核心：只有当模型说"P=0.85"时真实胜率确实接近85%，
//! 高置信度入场策略才能达到目标胜率。采用 Platt Scaling
//!（逻辑回归拟合 sigmoid(A*p + B)），简单稳定，适合小样本。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlattCalibrator {
    pub a: f64,
    pub b: f64,
}

impl PlattCalibrator {
    /// 在验证集上拟合 Platt 参数。
    /// `probs`: 模型原始输出概率；`labels`: 0/1 标签。
    pub fn fit(probs: &[f64], labels: &[f64]) -> Option<Self> {
        if probs.len() != labels.len() || probs.len() < 20 {
            return None;
        }
        // 转化为 logit 空间，避免边界问题
        let mut xs: Vec<f64> = Vec::with_capacity(probs.len());
        let mut ys: Vec<f64> = Vec::with_capacity(probs.len());
        for (&p, &y) in probs.iter().zip(labels.iter()) {
            let pc = p.clamp(1e-6, 1.0 - 1e-6);
            xs.push((pc / (1.0 - pc)).ln());
            ys.push(if y >= 0.5 { 1.0 } else { 0.0 });
        }

        // 简单逻辑回归（牛顿法或梯度下降）
        let (mut a, mut b) = (1.0_f64, 0.0_f64);
        let lr = 0.1;
        for _ in 0..500 {
            let mut ga = 0.0_f64;
            let mut gb = 0.0_f64;
            for (&x, &y) in xs.iter().zip(ys.iter()) {
                let z = a * x + b;
                let p = 1.0 / (1.0 + (-z).exp());
                let err = p - y;
                ga += err * x;
                gb += err;
            }
            let n = xs.len() as f64;
            a -= lr * ga / n;
            b -= lr * gb / n;
            // L2 正则防止过拟合
            a *= 0.999;
        }
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        Some(Self { a, b })
    }

    /// 校准单个概率
    pub fn calibrate(&self, p: f64) -> f64 {
        let pc = p.clamp(1e-6, 1.0 - 1e-6);
        let x = (pc / (1.0 - pc)).ln();
        let z = self.a * x + self.b;
        (1.0 / (1.0 + (-z).exp())).clamp(0.0, 1.0)
    }

    /// 恒等校准（未拟合时使用）
    pub fn identity() -> Self {
        Self { a: 1.0, b: 0.0 }
    }
}

/// 评估校准质量：按预测概率分桶，比较平均预测 vs 实际频率
pub fn calibration_report(probs: &[f64], labels: &[f64], n_bins: usize) -> Vec<(f64, f64, usize)> {
    let mut bins: Vec<(f64, f64, usize)> = vec![(0.0, 0.0, 0); n_bins];
    for (&p, &y) in probs.iter().zip(labels.iter()) {
        let b = ((p * n_bins as f64) as usize).min(n_bins - 1);
        bins[b].0 += p;
        bins[b].1 += if y >= 0.5 { 1.0 } else { 0.0 };
        bins[b].2 += 1;
    }
    bins.into_iter()
        .map(|(ps, ys, n)| {
            if n > 0 {
                (ps / n as f64, ys / n as f64, n)
            } else {
                (0.0, 0.0, 0)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rand::Rng;

    #[test]
    fn platt_recovers_calibration() {
        // 模拟系统性高估：真实 P=0.6 时模型输出 0.8
        let mut rng = StdRng::seed_from_u64(42);
        let mut probs = Vec::new();
        let mut labels = Vec::new();
        for _ in 0..500 {
            let true_p: f64 = rng.gen_range(0.3..0.7);
            let biased = (true_p + 0.15).min(0.95); // 系统性高估 0.15
            probs.push(biased);
            labels.push(if rng.gen_range(0.0..1.0) < true_p { 1.0 } else { 0.0 });
        }
        let cal = PlattCalibrator::fit(&probs, &labels).expect("fit");
        // 校准后，高 biased 概率应被下调
        let c = cal.calibrate(0.85);
        assert!(c < 0.85, "应下调高估的概率，得到 {c}");
        assert!(c > 0.4, "不应过度下调，得到 {c}");
        // 低概率应被上调（因为系统性高估，低 raw 可能对应更低的 true）
        // 至少保证单调性
        let c_low = cal.calibrate(0.4);
        assert!(c_low < c, "校准应保持单调性");
    }
}
