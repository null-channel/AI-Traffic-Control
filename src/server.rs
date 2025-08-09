use axum::{routing::{get, patch, post}, Json, Router};
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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}


