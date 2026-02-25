use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::routes::auth;
use crate::routes::errors;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TaskOverviewQuery {
    pub queued_limit: Option<i64>,
    pub running_limit: Option<i64>,
    pub recent_limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CrawlJobOut {
    pub id: String,
    pub job_type: String,
    pub fund_code: Option<String>,
    pub source_name: Option<String>,
    pub priority: i64,
    pub not_before: String,
    pub status: String,
    pub attempt: i64,
    pub last_error: Option<String>,
    pub last_ok_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct TaskJobOut {
    pub id: String,
    pub task_type: String,
    pub payload_json: String,
    pub priority: i64,
    pub not_before: String,
    pub status: String,
    pub attempt: i64,
    pub error: Option<String>,
    pub created_by: Option<i64>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct TaskRunOut {
    pub id: String,
    pub queue_type: String,
    pub job_id: String,
    pub job_type: String,
    pub fund_code: Option<String>,
    pub source_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskOverviewOut {
    pub crawl_queue: Vec<CrawlJobOut>,
    pub task_queue: Vec<TaskJobOut>,
    pub recent_jobs: Vec<TaskJobOut>,
    pub running: Vec<TaskRunOut>,
    pub recent: Vec<TaskRunOut>,
}

pub async fn overview(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(q): axum::extract::Query<TaskOverviewQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let queued_limit = q.queued_limit.unwrap_or(200).clamp(1, 2000);
    let running_limit = q.running_limit.unwrap_or(50).clamp(1, 500);
    let recent_limit = q.recent_limit.unwrap_or(20).clamp(1, 200);

    // 任务队列统一以 task_job/task_run 为准；crawl_job 属于内部细粒度队列，默认不在此处展开。
    let crawl_queue: Vec<CrawlJobOut> = Vec::new();

    let task_rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          task_type,
          payload_json,
          priority,
          CAST(not_before AS TEXT) as not_before,
          status,
          attempt,
          error,
          created_by,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM task_job
        WHERE status IN ('queued','running')
        ORDER BY priority DESC, not_before ASC
        LIMIT $1
        "#,
    )
    .bind(queued_limit)
    .fetch_all(pool)
    .await;

    let task_rows = match task_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut task_queue: Vec<TaskJobOut> = Vec::with_capacity(task_rows.len());
    for r in task_rows {
        task_queue.push(TaskJobOut {
            id: r.get("id"),
            task_type: r.get("task_type"),
            payload_json: r.get::<String, _>("payload_json"),
            priority: r.get("priority"),
            not_before: r.get("not_before"),
            status: r.get("status"),
            attempt: r.get("attempt"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            created_by: r.try_get::<Option<i64>, _>("created_by").ok().flatten(),
            started_at: r.try_get::<Option<String>, _>("started_at").ok().flatten(),
            finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        });
    }

    let recent_job_rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          task_type,
          payload_json,
          priority,
          CAST(not_before AS TEXT) as not_before,
          status,
          attempt,
          error,
          created_by,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM task_job
        WHERE status IN ('done','error')
        ORDER BY finished_at DESC NULLS LAST, updated_at DESC, created_at DESC
        LIMIT $1
        "#,
    )
    .bind(recent_limit)
    .fetch_all(pool)
    .await;

    let recent_job_rows = match recent_job_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut recent_jobs: Vec<TaskJobOut> = Vec::with_capacity(recent_job_rows.len());
    for r in recent_job_rows {
        recent_jobs.push(TaskJobOut {
            id: r.get("id"),
            task_type: r.get("task_type"),
            payload_json: r.get::<String, _>("payload_json"),
            priority: r.get("priority"),
            not_before: r.get("not_before"),
            status: r.get("status"),
            attempt: r.get("attempt"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            created_by: r.try_get::<Option<i64>, _>("created_by").ok().flatten(),
            started_at: r.try_get::<Option<String>, _>("started_at").ok().flatten(),
            finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        });
    }

    let running_rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          queue_type,
          CAST(job_id AS TEXT) as job_id,
          job_type,
          fund_code,
          source_name,
          status,
          error,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at
        FROM task_run
        WHERE status = 'running' AND queue_type = 'task_job'
        ORDER BY started_at DESC
        LIMIT $1
        "#,
    )
    .bind(running_limit)
    .fetch_all(pool)
    .await;

    let running_rows = match running_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut running: Vec<TaskRunOut> = Vec::with_capacity(running_rows.len());
    for r in running_rows {
        running.push(TaskRunOut {
            id: r.get("id"),
            queue_type: r.get("queue_type"),
            job_id: r.get("job_id"),
            job_type: r.get("job_type"),
            fund_code: r.try_get::<Option<String>, _>("fund_code").ok().flatten(),
            source_name: r.try_get::<Option<String>, _>("source_name").ok().flatten(),
            status: r.get("status"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            started_at: r.get("started_at"),
            finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
        });
    }

    let recent_rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          queue_type,
          CAST(job_id AS TEXT) as job_id,
          job_type,
          fund_code,
          source_name,
          status,
          error,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at
        FROM task_run
        WHERE finished_at IS NOT NULL AND status IN ('ok','error') AND queue_type = 'task_job'
        ORDER BY finished_at DESC
        LIMIT $1
        "#,
    )
    .bind(recent_limit)
    .fetch_all(pool)
    .await;

    let recent_rows = match recent_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut recent: Vec<TaskRunOut> = Vec::with_capacity(recent_rows.len());
    for r in recent_rows {
        recent.push(TaskRunOut {
            id: r.get("id"),
            queue_type: r.get("queue_type"),
            job_id: r.get("job_id"),
            job_type: r.get("job_type"),
            fund_code: r.try_get::<Option<String>, _>("fund_code").ok().flatten(),
            source_name: r.try_get::<Option<String>, _>("source_name").ok().flatten(),
            status: r.get("status"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            started_at: r.get("started_at"),
            finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
        });
    }

    (
        StatusCode::OK,
        Json(TaskOverviewOut {
            crawl_queue,
            task_queue,
            recent_jobs,
            running,
            recent,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct TaskLogsQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TaskLogLineOut {
    pub level: String,
    pub message: String,
    pub created_at: String,
}

pub async fn run_logs(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<TaskLogsQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let limit = q.limit.unwrap_or(500).clamp(1, 2000);
    let rows = sqlx::query(
        r#"
        SELECT
          level,
          message,
          CAST(created_at AS TEXT) as created_at
        FROM task_run_log
        WHERE CAST(run_id AS TEXT) = $1
        ORDER BY created_at ASC
        LIMIT $2
        "#,
    )
    .bind(run_id.trim())
    .bind(limit)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut out: Vec<TaskLogLineOut> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(TaskLogLineOut {
            level: r.get("level"),
            message: r.get("message"),
            created_at: r.get("created_at"),
        });
    }

    (StatusCode::OK, Json(out)).into_response()
}

#[derive(Debug, Serialize)]
pub struct TaskJobDetailOut {
    pub job: TaskJobOut,
    pub last_run: Option<TaskRunOut>,
}

pub async fn job_detail(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(job_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let job_row = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          task_type,
          payload_json,
          priority,
          CAST(not_before AS TEXT) as not_before,
          status,
          attempt,
          error,
          created_by,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM task_job
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(job_id.trim())
    .fetch_optional(pool)
    .await;

    let Some(r) = (match job_row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    }) else {
        return (StatusCode::NOT_FOUND, Json(json!({ "detail": "Not found." }))).into_response();
    };

    let job = TaskJobOut {
        id: r.get("id"),
        task_type: r.get("task_type"),
        payload_json: r.get::<String, _>("payload_json"),
        priority: r.get("priority"),
        not_before: r.get("not_before"),
        status: r.get("status"),
        attempt: r.get("attempt"),
        error: r.try_get::<Option<String>, _>("error").ok().flatten(),
        created_by: r.try_get::<Option<i64>, _>("created_by").ok().flatten(),
        started_at: r.try_get::<Option<String>, _>("started_at").ok().flatten(),
        finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    };

    let last_run_row = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          queue_type,
          CAST(job_id AS TEXT) as job_id,
          job_type,
          fund_code,
          source_name,
          status,
          error,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at
        FROM task_run
        WHERE queue_type = 'task_job' AND CAST(job_id AS TEXT) = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(job.id.as_str())
    .fetch_optional(pool)
    .await;

    let last_run = match last_run_row {
        Ok(Some(r)) => Some(TaskRunOut {
            id: r.get("id"),
            queue_type: r.get("queue_type"),
            job_id: r.get("job_id"),
            job_type: r.get("job_type"),
            fund_code: r.try_get::<Option<String>, _>("fund_code").ok().flatten(),
            source_name: r.try_get::<Option<String>, _>("source_name").ok().flatten(),
            status: r.get("status"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            started_at: r.get("started_at"),
            finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
        }),
        Ok(None) => None,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    (StatusCode::OK, Json(TaskJobDetailOut { job, last_run })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct TaskRunsQuery {
    pub limit: Option<i64>,
}

pub async fn job_runs(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(job_id): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<TaskRunsQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          queue_type,
          CAST(job_id AS TEXT) as job_id,
          job_type,
          fund_code,
          source_name,
          status,
          error,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at
        FROM task_run
        WHERE queue_type = 'task_job' AND CAST(job_id AS TEXT) = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
    )
    .bind(job_id.trim())
    .bind(limit)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut out: Vec<TaskRunOut> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(TaskRunOut {
            id: r.get("id"),
            queue_type: r.get("queue_type"),
            job_id: r.get("job_id"),
            job_type: r.get("job_type"),
            fund_code: r.try_get::<Option<String>, _>("fund_code").ok().flatten(),
            source_name: r.try_get::<Option<String>, _>("source_name").ok().flatten(),
            status: r.get("status"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            started_at: r.get("started_at"),
            finished_at: r.try_get::<Option<String>, _>("finished_at").ok().flatten(),
        });
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn job_logs(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(job_id): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<TaskLogsQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let run_row = sqlx::query(
        r#"
        SELECT CAST(id AS TEXT) as id
        FROM task_run
        WHERE queue_type = 'task_job' AND CAST(job_id AS TEXT) = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(job_id.trim())
    .fetch_optional(pool)
    .await;

    let run_id = match run_row {
        Ok(Some(r)) => Some(r.get::<String, _>("id")),
        Ok(None) => None,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let Some(run_id) = run_id else {
        return (StatusCode::OK, Json(Vec::<TaskLogLineOut>::new())).into_response();
    };

    run_logs(
        axum::extract::State(state),
        headers,
        axum::extract::Path(run_id),
        axum::extract::Query(q),
    )
    .await
}
