use axum::{routing::{get, post, delete}, Json, Router};
use axum::extract::Query;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use uuid::Uuid;

use crate::models::{LanguageModel, ModelRequest, OpenAICompatible, ModelSelector};
use crate::discovery::{list_files, search_files, read_file_under_root};
use crate::file_ops::{write_file_under_root, move_file_under_root, delete_file_under_root};
use crate::git_ops::{status as git_status, diff_porcelain as git_diff, add_all as git_add_all, commit as git_commit};
use crate::settings::{SessionSettings, SessionSettingsPatch};
use url::Url;
use metrics::Unit;
use crate::storage::{SqliteSessionRepository, SessionRepository};
use chrono::Utc;

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<SqliteSessionRepository>,
    pub model: Option<OpenAICompatible>,
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions", "method" => "POST"); }
    let settings = body.settings.unwrap_or_default();
    let id = state.repo.create_session(body.client_id.clone(), settings).await.expect("create session");
    Json(CreateSessionResponse { id })
}

async fn delete_session(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id", "method" => "DELETE"); }
    let ok = state.repo.delete_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ok {
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions", "method" => "GET"); }
    let ids = state.repo.list_sessions().await.unwrap_or_default();
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/settings", "method" => "GET"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match s { Some(sess) => Ok(Json(SessionSettingsResponse { settings: sess.settings })), None => Err(StatusCode::NOT_FOUND) }
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/history", "method" => "GET"); }
    let limit = q.limit.unwrap_or(50).min(200).max(1);
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;

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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/messages", "method" => "POST"); }
    // Resolve session and decide model
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let selected = ModelSelector::select(b.model.clone(), s.settings.default_model.clone(), None);

    // Append user message summary
    let user_msg = crate::session::Message {
        id: Uuid::new_v4(),
        role: b.role.clone().unwrap_or_else(|| "user".into()),
        content_summary: summarize(&b.content, 200),
        model_used: selected.clone(),
        created_at: Utc::now(),
    };
    state.repo.append_message(id, user_msg.clone()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Call model if configured
    if let Some(model) = &state.model {
        if let Some(model_name) = selected.clone() {
            let req = ModelRequest { model: model_name.clone(), prompt: b.content.clone(), temperature: s.settings.model_params.as_ref().and_then(|p| p.temperature), max_tokens: s.settings.model_params.as_ref().and_then(|p| p.max_tokens), top_p: s.settings.model_params.as_ref().and_then(|p| p.top_p) };
            match model.generate(req).await {
                Ok(r) => {
                    // store assistant message summary
                    let as_msg = crate::session::Message { id: Uuid::new_v4(), role: "assistant".into(), content_summary: summarize(&r.content, 200), model_used: Some(r.model.clone()), created_at: Utc::now() };
                    state.repo.append_message(id, as_msg).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
                Err(e) => {
                    state.repo.append_tool_event(id, crate::session::ToolEvent { id: Uuid::new_v4(), tool: "model".into(), summary: format!("error: {}", e), status: "error".into(), error: Some(e.to_string()), created_at: Utc::now() }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                }
            }
        }
    }

    let resp = PostMessageResponse { id: user_msg.id, role: user_msg.role, content_summary: user_msg.content_summary, model_used: selected };
    Ok(Json(resp))
}

#[derive(Debug, Deserialize)]
struct ListQuery { max: Option<usize> }

async fn list_session_files(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/discovery/list", "method" => "GET"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/discovery/search", "method" => "GET"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/discovery/read", "method" => "GET"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/files/write", "method" => "POST"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/files/move", "method" => "POST"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/files/delete", "method" => "POST"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let dry_run = b.dry_run.unwrap_or_else(|| s.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
    let res = delete_file_under_root(&root, &b.path, dry_run).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(res).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

async fn get_git_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/git/status", "method" => "GET"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let st = git_status(&root).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(st).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

async fn get_git_diff(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/git/diff", "method" => "GET"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let d = git_diff(&root).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::json!({"diff": d})))
}

async fn post_git_add_all(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/git/add_all", "method" => "POST"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
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
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/git/commit", "method" => "POST"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let root = s.settings.project_root.clone().ok_or(StatusCode::BAD_REQUEST)?;
    let oid = git_commit(&root, &b.message).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::json!({"commit": oid})))
}

async fn patch_session_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(patch): Json<SessionSettingsPatch>,
) -> Result<Json<SessionSettingsResponse>, StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/settings", "method" => "PATCH"); }
    let mut s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    s.settings.apply_patch(patch);
    state.repo.update_settings(id, s.settings.clone()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(SessionSettingsResponse { settings: s.settings }))
}

async fn healthz() -> Json<serde_json::Value> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/healthz", "method" => "GET"); }
    Json(serde_json::json!({"ok": true}))
}

#[derive(Debug, Deserialize)]
struct UrlIngestBody { url: String, max_bytes: Option<usize> }

fn is_allowed_host(allowlist: &Option<Vec<String>>, host: &str) -> bool {
    match allowlist {
        None => false,
        Some(list) => list.iter().any(|h| h == host),
    }
}

async fn fetch_and_extract(url: &str, max_bytes: usize) -> anyhow::Result<String> {
    let resp = reqwest::Client::new().get(url).send().await?;
    let status = resp.status();
    if !status.is_success() { anyhow::bail!("fetch failed: {}", status); }
    let bytes = resp.bytes().await?;
    let slice = if bytes.len() > max_bytes { &bytes[..max_bytes] } else { &bytes };
    let html = String::from_utf8_lossy(slice).to_string();
    let doc = scraper::Html::parse_document(&html);
    let selector = scraper::Selector::parse("body").unwrap();
    let mut text = String::new();
    for el in doc.select(&selector) {
        text.push_str(&el.text().collect::<Vec<_>>().join(" "));
        text.push('\n');
    }
    if text.is_empty() { Ok(html) } else { Ok(text) }
}

async fn ingest_url(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(b): Json<UrlIngestBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    { let _ = metrics::counter!("http.requests", "path" => "/v1/sessions/:id/context/url", "method" => "POST"); }
    let s = state.repo.get_session(id).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let parsed = Url::parse(&b.url).map_err(|_| StatusCode::BAD_REQUEST)?;
    let host = parsed.host_str().ok_or(StatusCode::BAD_REQUEST)?;
    if !is_allowed_host(&s.settings.network_allowlist, host) {
        return Err(StatusCode::FORBIDDEN);
    }
    let max_bytes = b.max_bytes.unwrap_or(256 * 1024).min(2 * 1024 * 1024);
    let content = fetch_and_extract(&b.url, max_bytes).await.map_err(|_| StatusCode::BAD_REQUEST)?;
    state.repo.append_tool_event(id, crate::session::ToolEvent {
        id: Uuid::new_v4(),
        tool: "url".into(),
        summary: format!("fetched {} ({} chars)", b.url, content.len()),
        status: "ok".into(),
        error: None,
        created_at: Utc::now(),
    }).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({"url": b.url, "content": content})))
}

pub async fn serve(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
    // Metrics setup
    metrics::describe_counter!("http.requests", Unit::Count, "HTTP requests by path and method");
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("install prometheus recorder");

    let app = Router::new()
        .route("/v1/healthz", get(healthz))
        .route("/metrics", get(move || async move { recorder.render() }))
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
        .route("/v1/sessions/:id/context/url", post(ingest_url))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}


