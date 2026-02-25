use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

pub fn database_kind_from_pool(pool: &sqlx::AnyPool) -> DatabaseKind {
    // 优先以已建立连接的 pool 为准，避免运行时 env 读取缺失/被覆盖导致误判。
    // sqlx AnyPool 没有公开 kind()；但可以从 connect_options() 拿到 URL 再推断。
    use sqlx::ConnectOptions;

    let url = pool.connect_options().to_url_lossy();
    database_kind_from_url(url.as_str())
}

fn sqlite_db_path_from_url(url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if !url.starts_with("sqlite:") {
        return None;
    }

    let mut rest = &url["sqlite:".len()..];
    rest = rest.split('?').next().unwrap_or("").trim();
    if rest.is_empty() {
        return None;
    }

    if rest.starts_with(":memory:") || rest.starts_with("//:memory:") {
        return None;
    }

    // sqlx sqlite 支持：
    // - sqlite:data.db
    // - sqlite://data.db
    // - sqlite:/abs/path.db
    // - sqlite:///abs/path.db
    let path = if let Some(abs) = rest.strip_prefix("///") {
        format!("/{abs}")
    } else if let Some(rel) = rest.strip_prefix("//") {
        rel.to_string()
    } else {
        rest.to_string()
    };

    if path.trim().is_empty() {
        return None;
    }

    let path = path.replace('/', std::path::MAIN_SEPARATOR_STR);
    Some(PathBuf::from(path))
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

pub fn ensure_sqlite_db_file(database_url: &str) -> std::io::Result<()> {
    let Some(path) = sqlite_db_path_from_url(database_url) else {
        return Ok(());
    };
    ensure_parent_dir(&path)?;
    if path.exists() {
        return Ok(());
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
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
