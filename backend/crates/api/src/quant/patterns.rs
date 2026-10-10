//! 技术形态识别（参考 Abu 项目整合的经典技术形态理论）。
//!
//! 在基金净值序列上识别：
//! - 头肩顶 / 头肩底（反转形态）
//! - 双顶 / 双底（反转形态）
//! - 三角整理（持续形态，含对称/上升/下降三角）
//! - 均线系统状态（多头/空头/纠缠）
//!
//! 输出形态信号，供策略层使用。基金净值波动小于个股，
//! 形态识别采用相对宽松的阈值。

/// 识别到的形态
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// 头肩顶（看跌反转）
    HeadShouldersTop { neckline: f64 },
    /// 头肩底（看涨反转）
    HeadShouldersBottom { neckline: f64 },
    /// 双顶（看跌）
    DoubleTop { level: f64 },
    /// 双底（看涨）
    DoubleBottom { level: f64 },
    /// 对称三角整理（方向待定）
    SymmetricalTriangle,
    /// 上升三角（偏多）
    AscendingTriangle { resistance: f64 },
    /// 下降三角（偏空）
    DescendingTriangle { support: f64 },
    /// 无明确形态
    None,
}

impl Pattern {
    /// 形态的方向性：+1 看多，-1 看空，0 中性
    pub fn bias(&self) -> i8 {
        match self {
            Pattern::HeadShouldersTop { .. } => -1,
            Pattern::HeadShouldersBottom { .. } => 1,
            Pattern::DoubleTop { .. } => -1,
            Pattern::DoubleBottom { .. } => 1,
            Pattern::SymmetricalTriangle => 0,
            Pattern::AscendingTriangle { .. } => 1,
            Pattern::DescendingTriangle { .. } => -1,
            Pattern::None => 0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Pattern::HeadShouldersTop { .. } => "头肩顶",
            Pattern::HeadShouldersBottom { .. } => "头肩底",
            Pattern::DoubleTop { .. } => "双顶",
            Pattern::DoubleBottom { .. } => "双底",
            Pattern::SymmetricalTriangle => "对称三角",
            Pattern::AscendingTriangle { .. } => "上升三角",
            Pattern::DescendingTriangle { .. } => "下降三角",
            Pattern::None => "无形态",
        }
    }
}

/// 摆动点
#[derive(Debug, Clone)]
struct SwingPoint {
    idx: usize,
    price: f64,
    is_high: bool,
}

/// 找出过去 n 日内的摆动高低点
fn swing_points(navs: &[(String, f64)], idx: usize, n: usize, k: usize) -> Vec<SwingPoint> {
    let start = idx.saturating_sub(n - 1);
    let prices: Vec<f64> = navs[start..=idx].iter().map(|(_, p)| *p).collect();
    let mut points = Vec::new();
    for i in k..prices.len().saturating_sub(k) {
        let p = prices[i];
        let is_high = prices[i - k..i].iter().all(|&x| x <= p) && prices[i + 1..=i + k].iter().all(|&x| x < p);
        let is_low = prices[i - k..i].iter().all(|&x| x >= p) && prices[i + 1..=i + k].iter().all(|&x| x > p);
        if is_high {
            points.push(SwingPoint { idx: start + i, price: p, is_high: true });
        } else if is_low {
            points.push(SwingPoint { idx: start + i, price: p, is_high: false });
        }
    }
    points
}

/// 在 navs[idx] 处识别形态（使用 idx 之前的数据，避免未来函数）
pub fn detect_pattern(navs: &[(String, f64)], idx: usize) -> Pattern {
    if idx < 120 || idx >= navs.len() {
        return Pattern::None;
    }
    let swings = swing_points(navs, idx, 120, 5);
    if swings.len() < 5 {
        return Pattern::None;
    }

    // 按时间取最近的摆动点
    let recent: Vec<&SwingPoint> = swings.iter().rev().take(9).collect();

    // 头肩顶/底：需要 5 个连续摆动点（高-低-高-低-高 或反之）
    if let Some(p) = detect_head_shoulders(&recent) {
        return p;
    }
    // 双顶/双底
    if let Some(p) = detect_double(&recent) {
        return p;
    }
    // 三角整理
    if let Some(p) = detect_triangle(navs, idx) {
        return p;
    }
    Pattern::None
}

fn detect_head_shoulders(recent: &[&SwingPoint]) -> Option<Pattern> {
    if recent.len() < 5 {
        return None;
    }
    // recent 是逆序（最新在前），反转为正序
    let pts: Vec<&SwingPoint> = recent.iter().rev().take(5).cloned().collect();
    // 头肩顶：高 低 高(头) 低 高，且头明显高于两肩
    if pts[0].is_high && !pts[1].is_high && pts[2].is_high && !pts[3].is_high && pts[4].is_high {
        let head = pts[2].price;
        let shoulder_avg = (pts[0].price + pts[4].price) / 2.0;
        let neckline = (pts[1].price + pts[3].price) / 2.0;
        // 头比肩高至少 3%，两肩高度差不超过 5%
        if head > shoulder_avg * 1.03
            && (pts[0].price - pts[4].price).abs() / shoulder_avg < 0.05
            && neckline < shoulder_avg
        {
            return Some(Pattern::HeadShouldersTop { neckline });
        }
    }
    // 头肩底：低 高 低(头) 高 低
    if !pts[0].is_high && pts[1].is_high && !pts[2].is_high && pts[3].is_high && !pts[4].is_high {
        let head = pts[2].price;
        let shoulder_avg = (pts[0].price + pts[4].price) / 2.0;
        let neckline = (pts[1].price + pts[3].price) / 2.0;
        if head < shoulder_avg * 0.97
            && (pts[0].price - pts[4].price).abs() / shoulder_avg < 0.05
            && neckline > shoulder_avg
        {
            return Some(Pattern::HeadShouldersBottom { neckline });
        }
    }
    None
}

fn detect_double(recent: &[&SwingPoint]) -> Option<Pattern> {
    if recent.len() < 3 {
        return None;
    }
    let pts: Vec<&SwingPoint> = recent.iter().rev().take(3).cloned().collect();
    // 双顶：高 低 高，两高接近
    if pts[0].is_high && !pts[1].is_high && pts[2].is_high {
        let avg = (pts[0].price + pts[2].price) / 2.0;
        if (pts[0].price - pts[2].price).abs() / avg < 0.03 && pts[1].price < avg * 0.97 {
            return Some(Pattern::DoubleTop { level: avg });
        }
    }
    // 双底：低 高 低
    if !pts[0].is_high && pts[1].is_high && !pts[2].is_high {
        let avg = (pts[0].price + pts[2].price) / 2.0;
        if (pts[0].price - pts[2].price).abs() / avg < 0.03 && pts[1].price > avg * 1.03 {
            return Some(Pattern::DoubleBottom { level: avg });
        }
    }
    None
}

fn detect_triangle(navs: &[(String, f64)], idx: usize) -> Option<Pattern> {
    // 取过去 60 日高低点序列，拟合上下趋势线
    let n = 60;
    if idx < n {
        return None;
    }
    let prices: Vec<f64> = navs[idx - n + 1..=idx].iter().map(|(_, p)| *p).collect();
    // 分段找高点趋势和低点趋势（每 10 日一段）
    let mut seg_highs = Vec::new();
    let mut seg_lows = Vec::new();
    for chunk in prices.chunks(10) {
        seg_highs.push(chunk.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        seg_lows.push(chunk.iter().cloned().fold(f64::INFINITY, f64::min));
    }
    if seg_highs.len() < 4 {
        return None;
    }
    let slope = |xs: &[f64]| -> f64 {
        let m = xs.len() as f64;
        let mx = (m - 1.0) / 2.0;
        let my = xs.iter().sum::<f64>() / m;
        let mut num = 0.0;
        let mut den = 0.0;
        for (i, y) in xs.iter().enumerate() {
            num += (i as f64 - mx) * (y - my);
            den += (i as f64 - mx).powi(2);
        }
        if den < 1e-12 {
            0.0
        } else {
            num / den / my // 相对斜率
        }
    };
    let hs = slope(&seg_highs);
    let ls = slope(&seg_lows);
    // 波动收敛：高低点差距缩小
    let first_range = seg_highs[0] - seg_lows[0];
    let last_range = seg_highs[seg_highs.len() - 1] - seg_lows[seg_lows.len() - 1];
    if first_range <= 0.0 || last_range / first_range > 0.7 {
        return None; // 未收敛
    }
    let resistance = seg_highs[seg_highs.len() - 1];
    let support = seg_lows[seg_lows.len() - 1];
    if hs < -0.005 && ls > 0.005 {
        Some(Pattern::SymmetricalTriangle)
    } else if hs.abs() < 0.005 && ls > 0.005 {
        Some(Pattern::AscendingTriangle { resistance })
    } else if hs < -0.005 && ls.abs() < 0.005 {
        Some(Pattern::DescendingTriangle { support })
    } else {
        None
    }
}

/// 均线系统状态
#[derive(Debug, Clone, PartialEq)]
pub enum MaState {
    Bullish, // 多头排列
    Bearish, // 空头排列
    Tangled, // 纠缠（震荡）
}

pub fn ma_state(navs: &[(String, f64)], idx: usize) -> MaState {
    use crate::quant::factors::{compute_all, FACTOR_NAMES};
    let f = compute_all(navs, idx);
    let align = FACTOR_NAMES
        .iter()
        .position(|&n| n == "ma_align")
        .map(|i| f[i])
        .unwrap_or(0.0);
    if align > 0.5 {
        MaState::Bullish
    } else if align < -0.5 {
        MaState::Bearish
    } else {
        MaState::Tangled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_runs_without_panic() {
        // 构造双底形态
        let mut navs: Vec<(String, f64)> = Vec::new();
        let mut p = 1.0;
        for i in 0..200 {
            // 制造 W 形
            let cycle = (i % 50) as f64;
            p = 1.0 + ((cycle - 25.0) / 25.0).powi(2) * 0.1;
            navs.push((format!("d{i}"), p));
        }
        let pat = detect_pattern(&navs, 199);
        // 不断言具体形态，只确保不 panic 且返回合法值
        let _ = pat.bias();
        let _ = pat.name();
    }

    #[test]
    fn ma_state_runs() {
        let navs: Vec<(String, f64)> = (0..300)
            .map(|i| (format!("d{i}"), 1.0 + i as f64 * 0.001))
            .collect();
        assert_eq!(ma_state(&navs, 299), MaState::Bullish);
    }
}
