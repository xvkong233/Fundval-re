use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use rand::{Rng, distributions::Alphanumeric};
use serde_json::Value;

#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    data: Arc<RwLock<BTreeMap<String, Value>>>,
}

impl ConfigStore {
    pub fn load() -> Self {
        let path = detect_config_path();
        let mut data = default_config();

        if path.exists()
            && let Ok(bytes) = fs::read(&path)
            && let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&bytes)
        {
            for (k, v) in map {
                data.insert(k, v);
            }
        }

        // 环境变量覆盖（与 Python 行为保持一致）
        if let Some(port) = std::env::var("PORT")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
        {
            data.insert("port".into(), Value::Number(serde_json::Number::from(port)));
        }
        if let Ok(db_type) = std::env::var("DB_TYPE") {
            data.insert("db_type".into(), Value::String(db_type));
        }
        if let Ok(allow_register) = std::env::var("ALLOW_REGISTER") {
            data.insert(
                "allow_register".into(),
                Value::Bool(allow_register.to_lowercase() == "true"),
            );
        }
        if let Ok(debug) = std::env::var("DEBUG") {
            data.insert("debug".into(), Value::Bool(debug.to_lowercase() == "true"));
        }

        Self {
            path,
            data: Arc::new(RwLock::new(data)),
        }
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        let guard = self.data.read().expect("config read lock");
        match guard.get(key) {
            Some(Value::Bool(v)) => *v,
            Some(Value::Number(n)) => n.as_i64().unwrap_or_default() != 0,
            _ => default,
        }
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        let guard = self.data.read().expect("config read lock");
        match guard.get(key) {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn get_i64(&self, key: &str, default: i64) -> i64 {
        let guard = self.data.read().expect("config read lock");
        match guard.get(key) {
            Some(Value::Number(n)) => n.as_i64().unwrap_or(default),
            Some(Value::String(s)) => s.parse::<i64>().unwrap_or(default),
            Some(Value::Bool(b)) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            _ => default,
        }
    }

    pub fn set_bool(&self, key: &str, value: bool) {
        let mut guard = self.data.write().expect("config write lock");
        guard.insert(key.to_string(), Value::Bool(value));
    }

    pub fn set_string(&self, key: &str, value: Option<String>) {
        let mut guard = self.data.write().expect("config write lock");
        match value {
            None => {
                guard.insert(key.to_string(), Value::Null);
            }
            Some(v) => {
                guard.insert(key.to_string(), Value::String(v));
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        // 优先保存到 /app/config/config.json；本地开发则保存到探测路径
        let path = preferred_save_path(&self.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let guard = self.data.read().expect("config read lock");
        let json = serde_json::to_vec_pretty(&*guard).expect("serialize config");
        fs::write(path, json)
    }

    pub fn system_initialized(&self) -> bool {
        self.get_bool("system_initialized", false)
    }

    pub fn allow_register(&self) -> bool {
        self.get_bool("allow_register", false)
    }

    pub fn set_system_initialized(&self, value: bool) {
        self.set_bool("system_initialized", value);
    }

    pub fn set_allow_register(&self, value: bool) {
        self.set_bool("allow_register", value);
    }

    /// 等价于 Python 版 `get_bootstrap_key()`：
    /// - 若未生成，则生成 64 字符高熵随机字符串并持久化
    /// - 若已初始化，则仍返回当前值（但 verify 会直接失败）
    pub fn get_or_generate_bootstrap_key(&self) -> Option<String> {
        if let Some(existing) = self.get_string("bootstrap_key") {
            return Some(existing);
        }

        // 如果值是 null 或不存在，生成并保存
        let key: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        self.set_string("bootstrap_key", Some(key.clone()));
        let _ = self.save();
        Some(key)
    }

    pub fn verify_bootstrap_key(&self, key: Option<&str>) -> bool {
        if self.system_initialized() {
            return false;
        }
        let expected = self.get_or_generate_bootstrap_key();
        match (expected, key) {
            (Some(expected), Some(key)) => expected == key,
            _ => false,
        }
    }
}

fn default_config() -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    m.insert("port".into(), Value::Number(8001.into()));
    // 本仓库 Rust 移植版只使用 Postgres（sqlx 仅启用 postgres feature）。
    // 该字段保留是为了对齐原项目配置结构，但默认值不应再指向 sqlite 以免误导。
    m.insert("db_type".into(), Value::String("postgres".into()));
    m.insert("db_config".into(), Value::Object(Default::default()));
    m.insert("allow_register".into(), Value::Bool(false));
    m.insert("system_initialized".into(), Value::Bool(false));
    m.insert("debug".into(), Value::Bool(false));
    m.insert("estimate_cache_ttl".into(), Value::Number(5.into()));
    m.insert("sources_health_probe".into(), Value::Bool(true));
    m.insert("tushare_token".into(), Value::Null);
    m
}

fn detect_config_path() -> PathBuf {
    // 与 Python 行为对齐：优先 /app/config/config.json，否则回退到本地 backend 目录下的 config.json
    let preferred = PathBuf::from("/app/config/config.json");
    if preferred.exists() {
        return preferred;
    }
    PathBuf::from("config.json")
}

fn preferred_save_path(fallback: &Path) -> PathBuf {
    let preferred_dir = PathBuf::from("/app/config");
    if preferred_dir.exists() {
        return preferred_dir.join("config.json");
    }
    fallback.to_path_buf()
}
