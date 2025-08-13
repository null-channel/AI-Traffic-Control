use std::path::PathBuf;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite, sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous}, Row};
use uuid::Uuid;

use crate::session::{Session, Message, ToolEvent};
use crate::settings::SessionSettings;

#[derive(Clone)]
pub struct SqliteSessionRepository {
    pool: Pool<Sqlite>,
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create_session(&self, client_id: Option<String>, settings: SessionSettings) -> anyhow::Result<Uuid>;
    async fn delete_session(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn list_sessions(&self) -> anyhow::Result<Vec<Uuid>>;
    async fn get_session(&self, id: Uuid) -> anyhow::Result<Option<Session>>;
    async fn update_settings(&self, id: Uuid, settings: SessionSettings) -> anyhow::Result<()>;
    async fn append_message(&self, id: Uuid, msg: Message) -> anyhow::Result<()>;
    async fn append_tool_event(&self, id: Uuid, ev: ToolEvent) -> anyhow::Result<()>;
}

impl SqliteSessionRepository {
    pub async fn initialize(database_url: Option<String>) -> anyhow::Result<Self> {
        let url = match database_url {
            Some(u) => u,
            None => resolve_default_db_url()?,
        };
        let options = url.parse::<SqliteConnectOptions>()?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full);
        let pool = Pool::<Sqlite>::connect_with(options).await?;
        // busy_timeout via PRAGMA
        sqlx::query("PRAGMA busy_timeout = 5000;").execute(&pool).await?;
        // apply migrations
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    #[cfg(test)]
    pub fn pool(&self) -> &Pool<Sqlite> { &self.pool }
}

fn resolve_default_db_url() -> anyhow::Result<String> {
    let base = std::env::var("XDG_DATA_HOME").ok().map(PathBuf::from).unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".local").join("share")
    });
    let dir = base.join("air_traffic_control");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("atc.db");
    Ok(format!("sqlite://{}", path.to_string_lossy()))
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn create_session(&self, client_id: Option<String>, settings: SessionSettings) -> anyhow::Result<Uuid> {
        let id = Uuid::new_v4();
        let now: DateTime<Utc> = Utc::now();
        let settings_json = serde_json::to_string(&settings)?;
        sqlx::query("INSERT INTO sessions (id, client_id, created_at, settings_json) VALUES (?1, ?2, ?3, ?4)")
            .bind(id.to_string())
            .bind(client_id)
            .bind(now.to_rfc3339())
            .bind(settings_json)
            .execute(&self.pool).await?;
        Ok(id)
    }

    async fn delete_session(&self, id: Uuid) -> anyhow::Result<bool> {
        let res = sqlx::query("DELETE FROM sessions WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected() > 0)
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<Uuid>> {
        let rows = sqlx::query("SELECT id FROM sessions ORDER BY created_at DESC").fetch_all(&self.pool).await?;
        let ids = rows.into_iter().filter_map(|r| {
            let id_str: String = r.get::<String, _>("id");
            Uuid::parse_str(&id_str).ok()
        }).collect();
        Ok(ids)
    }

    async fn get_session(&self, id: Uuid) -> anyhow::Result<Option<Session>> {
        use sqlx::Row;
        let row = sqlx::query("SELECT id, client_id, created_at, settings_json FROM sessions WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;
        let Some(r) = row else { return Ok(None) };
        let settings_json: String = r.get("settings_json");
        let settings: SessionSettings = serde_json::from_str(&settings_json)?;
        let messages_rows = sqlx::query("SELECT id, role, content_summary, model_used, created_at FROM messages WHERE session_id = ?1 ORDER BY created_at ASC")
            .bind(id.to_string())
            .fetch_all(&self.pool).await?;
        let tool_rows = sqlx::query("SELECT id, tool, summary, status, error, created_at FROM tool_events WHERE session_id = ?1 ORDER BY created_at ASC")
            .bind(id.to_string())
            .fetch_all(&self.pool).await?;
        let messages = messages_rows.into_iter().map(|m| {
            let id_str: String = m.get("id");
            let role: String = m.get("role");
            let content_summary: String = m.get("content_summary");
            let model_used: Option<String> = m.try_get("model_used").ok();
            let created_at: String = m.get("created_at");
            Message {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                role,
                content_summary,
                model_used,
                created_at: DateTime::parse_from_rfc3339(&created_at).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
            }
        }).collect();
        let tool_history = tool_rows.into_iter().map(|t| {
            let id_str: String = t.get("id");
            let tool: String = t.get("tool");
            let summary: String = t.get("summary");
            let status: String = t.get("status");
            let error: Option<String> = t.try_get("error").ok();
            let created_at: String = t.get("created_at");
            ToolEvent {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                tool,
                summary,
                status,
                error,
                created_at: DateTime::parse_from_rfc3339(&created_at).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
            }
        }).collect();
        let id_parsed = {
            let id_str: String = r.get("id");
            Uuid::parse_str(&id_str).unwrap()
        };
        let client_id: Option<String> = r.try_get("client_id").ok();
        let created_at = {
            let s: String = r.get("created_at");
            DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now())
        };
        let session = Session { id: id_parsed, client_id, created_at, messages, tool_history, settings };
        Ok(Some(session))
    }

    async fn update_settings(&self, id: Uuid, settings: SessionSettings) -> anyhow::Result<()> {
        let settings_json = serde_json::to_string(&settings)?;
        sqlx::query("UPDATE sessions SET settings_json = ?1 WHERE id = ?2")
            .bind(settings_json)
            .bind(id.to_string())
            .execute(&self.pool).await?;
        Ok(())
    }

    async fn append_message(&self, id: Uuid, msg: Message) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO messages (id, session_id, role, content_summary, model_used, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
            .bind(msg.id.to_string())
            .bind(id.to_string())
            .bind(msg.role)
            .bind(msg.content_summary)
            .bind(msg.model_used)
            .bind(msg.created_at.to_rfc3339())
            .execute(&self.pool).await?;
        Ok(())
    }

    async fn append_tool_event(&self, id: Uuid, ev: ToolEvent) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO tool_events (id, session_id, tool, summary, status, error, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")
            .bind(ev.id.to_string())
            .bind(id.to_string())
            .bind(ev.tool)
            .bind(ev.summary)
            .bind(ev.status)
            .bind(ev.error)
            .bind(ev.created_at.to_rfc3339())
            .execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use sqlx::Row;

    #[tokio::test]
    async fn create_get_list_delete_session_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let url = format!("sqlite://{}", path.to_string_lossy());
        let repo = SqliteSessionRepository::initialize(Some(url)).await.unwrap();

        let settings = SessionSettings::default();
        let id = repo.create_session(Some("client-1".into()), settings.clone()).await.unwrap();

        let list = repo.list_sessions().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], id);

        let got = repo.get_session(id).await.unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.client_id.as_deref(), Some("client-1"));
        assert_eq!(got.settings, settings);
        assert!(got.messages.is_empty());
        assert!(got.tool_history.is_empty());

        let ok = repo.delete_session(id).await.unwrap();
        assert!(ok);
        let list2 = repo.list_sessions().await.unwrap();
        assert!(list2.is_empty());
    }

    #[tokio::test]
    async fn append_history_and_update_settings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let url = format!("sqlite://{}", path.to_string_lossy());
        let repo = SqliteSessionRepository::initialize(Some(url)).await.unwrap();
        let id = repo.create_session(None, SessionSettings::default()).await.unwrap();

        let msg = Message {
            id: Uuid::new_v4(),
            role: "user".into(),
            content_summary: "hello".into(),
            model_used: None,
            created_at: Utc::now(),
        };
        repo.append_message(id, msg.clone()).await.unwrap();

        let ev = ToolEvent {
            id: Uuid::new_v4(),
            tool: "test".into(),
            summary: "ran".into(),
            status: "ok".into(),
            error: None,
            created_at: Utc::now(),
        };
        repo.append_tool_event(id, ev.clone()).await.unwrap();

        let mut new_settings = SessionSettings::default();
        new_settings.project_root = Some("/tmp".into());
        repo.update_settings(id, new_settings.clone()).await.unwrap();

        let got = repo.get_session(id).await.unwrap().unwrap();
        assert_eq!(got.messages.len(), 1);
        assert_eq!(got.messages[0].content_summary, "hello");
        assert_eq!(got.tool_history.len(), 1);
        assert_eq!(got.tool_history[0].tool, "test");
        assert_eq!(got.settings.project_root.as_deref(), Some("/tmp"));
    }

    #[tokio::test]
    async fn pragmas_and_migrations_applied() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let url = format!("sqlite://{}", path.to_string_lossy());
        let repo = SqliteSessionRepository::initialize(Some(url)).await.unwrap();

        // Check WAL mode
        let row = sqlx::query("PRAGMA journal_mode;").fetch_one(repo.pool()).await.unwrap();
        let mode: String = row.get(0);
        assert!(mode.eq_ignore_ascii_case("wal"), "journal_mode should be WAL, got {}", mode);

        // Check busy_timeout
        let row = sqlx::query("PRAGMA busy_timeout;").fetch_one(repo.pool()).await.unwrap();
        let timeout: i64 = row.get(0);
        assert!(timeout >= 5000, "busy_timeout should be at least 5000, got {}", timeout);

        // Migrations idempotent: re-run initialize on same file
        let _repo2 = SqliteSessionRepository::initialize(Some(format!("sqlite://{}", path.to_string_lossy()))).await.unwrap();
    }
}


