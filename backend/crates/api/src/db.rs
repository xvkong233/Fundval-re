use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

pub fn database_kind_from_url(url: &str) -> DatabaseKind {
    let url = url.trim();
    if url.starts_with("sqlite:") {
        return DatabaseKind::Sqlite;
    }
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        return DatabaseKind::Postgres;
    }
    // 保守：默认按 Postgres 处理，避免把未知 scheme 误判为 sqlite 文件
    DatabaseKind::Postgres
}

pub fn data_dir() -> PathBuf {
    explicit_data_dir().unwrap_or_else(|| PathBuf::from("data"))
}

pub fn explicit_data_dir() -> Option<PathBuf> {
    let Ok(v) = std::env::var("FUNDVAL_DATA_DIR") else {
        return None;
    };
    let v = v.trim();
    if v.is_empty() {
        None
    } else {
        Some(PathBuf::from(v))
    }
}

pub fn default_sqlite_db_path() -> PathBuf {
    data_dir().join("fundval.sqlite")
}

pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn default_database_url() -> String {
    // 发行版默认 sqlite；Docker/生产一般会显式提供 DATABASE_URL（Postgres）。
    let path = default_sqlite_db_path();
    // 仅确保目录存在，sqlite 文件由驱动在连接时创建
    let _ = ensure_parent_dir(&path);

    // sqlx sqlite 支持：sqlite:data.db / sqlite://data.db / sqlite:///abs/path
    // 这里选用最兼容的相对路径形式，避免 Windows 路径编码/盘符问题。
    let rel = path.to_string_lossy().replace('\\', "/");
    format!("sqlite:{rel}")
}

pub fn resolve_database_url() -> (String, DatabaseKind) {
    let url = std::env::var("DATABASE_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_database_url);
    let kind = database_kind_from_url(&url);
    (url, kind)
}
