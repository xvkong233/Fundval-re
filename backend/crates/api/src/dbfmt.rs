use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};

pub fn parse_datetime_utc(raw: &str) -> Option<DateTime<Utc>> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    // 先尝试 RFC3339（如 2024-01-01T10:00:00Z）
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // 兼容 Postgres TIMESTAMPTZ cast to text（常见：2024-01-01 10:00:00+00 / +00:00）
    if let Some((left, tz)) = s.rsplit_once(|c| c == '+' || c == '-') {
        let sign = s.as_bytes()[left.len()] as char;
        let tz = tz.trim();
        // tz 可能是 "00" / "00:00" / "0800" / "08:00"
        let tz_norm = match tz.len() {
            2 => format!("{tz}:00"),
            4 if !tz.contains(':') => format!("{}:{}", &tz[0..2], &tz[2..4]),
            _ => tz.to_string(),
        };
        let candidate = format!(
            "{}T{}{}{}",
            left.trim().split_whitespace().next().unwrap_or(""),
            left.trim().split_whitespace().nth(1).unwrap_or(""),
            sign,
            tz_norm
        );
        if let Ok(dt) = DateTime::parse_from_rfc3339(&candidate) {
            return Some(dt.with_timezone(&Utc));
        }
    }

    // 兼容 SQLite CURRENT_TIMESTAMP（UTC）：2024-01-01 10:00:00(.fff)
    for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            let dt = DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc);
            return Some(dt);
        }
    }

    None
}

pub fn datetime_to_rfc3339(raw: &str) -> String {
    match parse_datetime_utc(raw) {
        Some(dt) => dt.to_rfc3339_opts(SecondsFormat::AutoSi, false),
        None => raw.trim().to_string(),
    }
}
