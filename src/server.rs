use axum::{routing::{get, patch, post, delete}, Json, Router};
use axum::extract::Query;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::session::Session;
use crate::discovery::{list_files, search_files, read_file_under_root};
use crate::file_ops::{write_file_under_root, move_file_under_root, delete_file_under_root};
use crate::git_ops::{status as git_status, diff_porcelain as git_diff, add_all as git_add_all, commit as git_commit};
use crate::settings::{SessionSettings, SessionSettingsPatch};

#[derive(Clone, Default)]
pub struct AppState {
    pub sessions: Arc<RwLock<Vec<Session>>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionBody {
    pub client_id: Option<String>,
    pub settings: Option<SessionSettings>,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub id: Uuid,
}

async fn create_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<CreateSessionBody>,
) -> Json<CreateSessionResponse> {
    let settings = body.settings.unwrap_or_default();
    let mut sessions = state.sessions.write().await;
    let session = Session::new(body.client_id, settings);
    let id = session.id;
    sessions.push(session);
    Json(CreateSessionResponse { id })
}

async fn delete_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let mut sessions = state.sessions.write().await;
    let before = sessions.len();
    sessions.retain(|s| s.id != id);
    if sessions.len() < before {
        Ok(axum::http::StatusCode::NO_CONTENT)
    } else {
        Err(axum::http::StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<Uuid>,
}

async fn list_sessions(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<ListSessionsResponse> {
    let sessions = state.sessions.read().await;
    let ids = sessions.iter().map(|s| s.id).collect();
    Json(ListSessionsResponse { sessions: ids })
}

#[derive(Debug, Serialize)]
struct SessionSettingsResponse {
    settings: SessionSettings,
}

async fn get_session_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<SessionSettingsResponse>, StatusCode> {
    let sessions = state.sessions.read().await;
    if let Some(s) = sessions.iter().find(|s| s.id == id) {
        Ok(Json(SessionSettingsResponse { settings: s.settings.clone() }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, serde::Deserialize)]
struct HistoryQuery {
    kind: String,            // "messages" | "tools"
    cursor: Option<usize>,   // offset
    limit: Option<usize>,    // page size
}

#[derive(Debug, serde::Serialize)]
struct HistoryResponse {
    kind: String,
    items: serde_json::Value,
    next_cursor: Option<usize>,
}

fn paginate<T: Clone>(data: &[T], cursor: Option<usize>, limit: usize) -> (Vec<T>, Option<usize>) {
    let start = cursor.unwrap_or(0);
    if start >= data.len() { return (Vec::new(), None); }
    let end = (start + limit).min(data.len());
    let page = data[start..end].to_vec();
    let next = if end < data.len() { Some(end) } else { None };
    (page, next)
}

async fn get_session_history(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, StatusCode> {
    let limit = q.limit.unwrap_or(50).min(200).max(1);
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;

    match q.kind.as_str() {
        "messages" => {
            let (items, next) = paginate(&s.messages, q.cursor, limit);
            let items = serde_json::to_value(items).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(HistoryResponse { kind: "messages".into(), items, next_cursor: next }))
        }
        "tools" => {
            let (items, next) = paginate(&s.tool_history, q.cursor, limit);
            let items = serde_json::to_value(items).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(HistoryResponse { kind: "tools".into(), items, next_cursor: next }))
        }
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

#[derive(Debug, Deserialize)]
struct PostMessageBody { role: Option<String>, content: String, model: Option<String> }

#[derive(Debug, Serialize)]
struct PostMessageResponse { id: Uuid, role: String, content_summary: String, model_used: Option<String> }

fn summarize(content: &str, max: usize) -> String {
    if content.len() <= max { content.to_string() } else { format!("{}…", &content[..max]) }
}

async fn post_session_message(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(b): Json<PostMessageBody>,
) -> Result<Json<PostMessageResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    let s = sessions.iter_mut().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let msg = crate::session::Message {
        id: Uuid::new_v4(),
        role: b.role.clone().unwrap_or_else(|| "user".into()),
        content_summary: summarize(&b.content, 200),
        model_used: b.model.clone(),
        created_at: chrono::Utc::now(),
    };
    let resp = PostMessageResponse { id: msg.id, role: msg.role.clone(), content_summary: msg.content_summary.clone(), model_used: msg.model_used.clone() };
    s.messages.push(msg);
    Ok(Json(resp))
}

#[derive(Debug, Deserialize)]
struct ListQuery { max: Option<usize> }

async fn list_session_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let items = list_files(&root, q.max.unwrap_or(500));
    let v = serde_json::to_value(items).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(v))
}

#[derive(Debug, Deserialize)]
struct SearchQuery { pattern: String, max: Option<usize> }

async fn search_session_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let items = search_files(&root, &q.pattern, q.max.unwrap_or(500));
    let v = serde_json::to_value(items).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(v))
}

#[derive(Debug, Deserialize)]
struct ReadQuery { path: String, max_bytes: Option<usize> }

async fn read_session_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Query(q): Query<ReadQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let content = read_file_under_root(&root, &q.path, q.max_bytes.unwrap_or(64 * 1024))
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::json!({"path": q.path, "content": content})))
}

#[derive(Debug, Deserialize)]
struct WriteBody { path: String, content: String, create: Option<bool>, dry_run: Option<bool>, preview_bytes: Option<usize> }

async fn write_session_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(b): Json<WriteBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let dry_run = b.dry_run.unwrap_or_else(|| s.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
    let res = write_file_under_root(&root, &b.path, &b.content, b.create.unwrap_or(true), dry_run, b.preview_bytes.unwrap_or(1024))
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(res).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

#[derive(Debug, Deserialize)]
struct MoveBody { from: String, to: String, dry_run: Option<bool> }

async fn move_session_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(b): Json<MoveBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let dry_run = b.dry_run.unwrap_or_else(|| s.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
    let res = move_file_under_root(&root, &b.from, &b.to, dry_run).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(res).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

#[derive(Debug, Deserialize)]
struct DeleteBody { path: String, dry_run: Option<bool> }

async fn delete_session_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(b): Json<DeleteBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let dry_run = b.dry_run.unwrap_or_else(|| s.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
    let res = delete_file_under_root(&root, &b.path, dry_run).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(res).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

async fn get_git_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let st = git_status(&root).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(st).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

async fn get_git_diff(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let d = git_diff(&root).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::json!({"diff": d})))
}

async fn post_git_add_all(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    git_add_all(&root).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Debug, Deserialize)]
struct CommitBody { message: String }

async fn post_git_commit(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(b): Json<CommitBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let sessions = state.sessions.read().await;
    let s = sessions.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let oid = git_commit(&root, &b.message).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::json!({"commit": oid})))
}

async fn patch_session_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(patch): Json<SessionSettingsPatch>,
) -> Result<Json<SessionSettingsResponse>, StatusCode> {
    let mut sessions = state.sessions.write().await;
    if let Some(s) = sessions.iter_mut().find(|s| s.id == id) {
        s.settings.apply_patch(patch);
        Ok(Json(SessionSettingsResponse { settings: s.settings.clone() }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true}))
}

pub async fn serve(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/v1/healthz", get(healthz))
        .route("/v1/sessions", post(create_session).get(list_sessions))
        .route("/v1/sessions/:id/settings", get(get_session_settings).patch(patch_session_settings))
        .route("/v1/sessions/:id", delete(delete_session))
        .route("/v1/sessions/:id/messages", post(post_session_message))
        .route("/v1/sessions/:id/history", get(get_session_history))
        .route("/v1/sessions/:id/discovery/list", get(list_session_files))
        .route("/v1/sessions/:id/discovery/search", get(search_session_files))
        .route("/v1/sessions/:id/discovery/read", get(read_session_file))
        .route("/v1/sessions/:id/files/write", post(write_session_file))
        .route("/v1/sessions/:id/files/move", post(move_session_file))
        .route("/v1/sessions/:id/files/delete", post(delete_session_file))
        .route("/v1/sessions/:id/git/status", get(get_git_status))
        .route("/v1/sessions/:id/git/diff", get(get_git_diff))
        .route("/v1/sessions/:id/git/add_all", post(post_git_add_all))
        .route("/v1/sessions/:id/git/commit", post(post_git_commit))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}


