use axum::{routing::{get, patch, post}, Json, Router};
use axum::extract::Query;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::session::Session;
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

pub async fn serve(addr: SocketAddr, state: AppState) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/v1/sessions", post(create_session).get(list_sessions))
        .route("/v1/sessions/:id/settings", get(get_session_settings).patch(patch_session_settings))
        .route("/v1/sessions/:id/history", get(get_session_history))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}


