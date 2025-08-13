use crate::storage::SessionRepository;
use crate::session::ToolEvent;
use crate::discovery::read_file_under_root;
use crate::server::{fetch_and_extract, is_allowed_host};
use chrono::Utc;
use uuid::Uuid;
use serde_json::json;
use crate::agent::tools::{ToolRegistry, ToolContext as ToolsContext, Tool};

pub struct AgentContext<'a, R: SessionRepository> {
    pub repo: &'a R,
}

pub enum EngineCommand<'a> {
    IncludeFile { session_id: Uuid, project_root: &'a str, path: &'a str, max_bytes: usize },
    IncludeUrl { session_id: Uuid, allowlist: Option<&'a Vec<String>>, url: &'a str, max_bytes: usize },
    AddRuleSystem { session_id: Uuid, name: &'a str, content: &'a str },
    AddRuleRepo { session_id: Uuid, project_root: &'a str, name: &'a str, content: &'a str, repo_dir: &'a str },
}

pub async fn execute<R: SessionRepository>(ctx: AgentContext<'_, R>, cmd: EngineCommand<'_>) -> anyhow::Result<String> {
    match cmd {
        EngineCommand::IncludeFile { session_id, project_root, path, max_bytes } => {
            let content = read_file_under_root(project_root, path, max_bytes)?;
            ctx.repo.add_context_item(session_id, "file", path, &content, content.len() as i64).await?;
            ctx.repo.append_tool_event(session_id, ToolEvent { id: Uuid::new_v4(), tool: "include_file".into(), summary: format!("included {} ({} chars)", path, content.len()), status: "ok".into(), error: None, created_at: Utc::now() }).await?;
            Ok(format!("file:{} bytes:{}", path, content.len()))
        }
        EngineCommand::IncludeUrl { session_id, allowlist, url, max_bytes } => {
            let parsed = url::Url::parse(url)?;
            let host = parsed.host_str().ok_or_else(|| anyhow::anyhow!("invalid host"))?;
            let allowlist_opt = allowlist.cloned();
            if !is_allowed_host(&allowlist_opt, host) {
                anyhow::bail!("forbidden host");
            }
            let content = fetch_and_extract(url, max_bytes).await?;
            ctx.repo.add_context_item(session_id, "url", url, &content, content.len() as i64).await?;
            ctx.repo.append_tool_event(session_id, ToolEvent { id: Uuid::new_v4(), tool: "include_url".into(), summary: format!("included {} ({} chars)", url, content.len()), status: "ok".into(), error: None, created_at: Utc::now() }).await?;
            Ok(format!("url:{} bytes:{}", url, content.len()))
        }
        EngineCommand::AddRuleSystem { session_id, name, content } => {
            ctx.repo.upsert_rule(name, content).await?;
            ctx.repo.append_tool_event(session_id, ToolEvent { id: Uuid::new_v4(), tool: "add_rule".into(), summary: format!("system rule upserted: {}", name), status: "ok".into(), error: None, created_at: Utc::now() }).await?;
            Ok(format!("system rule:{}", name))
        }
        EngineCommand::AddRuleRepo { session_id, project_root, name, content, repo_dir } => {
            let path = std::path::Path::new(project_root).join(repo_dir).join(format!("{}.md", slugify(name)));
            let parent = path.parent().unwrap_or(std::path::Path::new(project_root)).to_path_buf();
            std::fs::create_dir_all(&parent)?;
            std::fs::write(&path, content.as_bytes())?;
            ctx.repo.append_tool_event(session_id, ToolEvent { id: Uuid::new_v4(), tool: "add_rule".into(), summary: format!("repo rule written: {}", path.display()), status: "ok".into(), error: None, created_at: Utc::now() }).await?;
            Ok(format!("repo rule:{}", path.display()))
        }
    }
}

pub async fn dispatch_tool<R: SessionRepository>(ctx: AgentContext<'_, R>, session_id: Uuid, tool_name: &str, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let sess = ctx.repo.get_session(session_id).await?.ok_or_else(|| anyhow::anyhow!("session not found"))?;
    let registry = ToolRegistry::with_default_tools();
    let tool = registry.get(tool_name).ok_or_else(|| anyhow::anyhow!("unknown tool"))?;
    let tctx = ToolsContext { repo: ctx.repo, session_id, settings: &sess.settings };
    let res = tool.run(tctx, args).await?;
    ctx.repo.append_tool_event(session_id, ToolEvent { id: Uuid::new_v4(), tool: tool.name().into(), summary: res.summary.clone(), status: "ok".into(), error: None, created_at: Utc::now() }).await?;
    Ok(json!({ "summary": res.summary, "data": res.data }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::SqliteSessionRepository;
    use crate::settings::SessionSettings;
    use std::fs;

    async fn setup_session_with_root() -> (SqliteSessionRepository, Uuid, String, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let root = dir.path().to_string_lossy().to_string();
        let db_path = dir.path().join("test.db");
        let url = format!("sqlite://{}", db_path.to_string_lossy());
        let repo = SqliteSessionRepository::initialize(Some(url)).await.unwrap();
        let sid = repo.create_session(None, SessionSettings::default()).await.unwrap();
        let mut s = repo.get_session(sid).await.unwrap().unwrap();
        s.settings.project_root = Some(root.clone());
        repo.update_settings(sid, s.settings.clone()).await.unwrap();
        (repo, sid, root, dir)
    }

    #[tokio::test]
    async fn tool_include_file_and_context_item() {
        let (repo, sid, root, _dir) = setup_session_with_root().await;
        let file_path = std::path::Path::new(&root).join("a.txt");
        fs::write(&file_path, b"hello world").unwrap();
        let ctx = AgentContext { repo: &repo };
        let v = dispatch_tool(ctx, sid, "include_file", serde_json::json!({"path": "a.txt", "max_bytes": 64})).await.unwrap();
        assert!(v["summary"].as_str().unwrap().contains("file:a.txt"));
        // verify context_items increment via direct query
        use sqlx::Row;
        let row = sqlx::query("SELECT count(*) as c FROM context_items WHERE session_id = ?1")
            .bind(sid.to_string()).fetch_one(repo.pool()).await.unwrap();
        let c: i64 = row.get::<i64, _>("c");
        assert_eq!(c, 1);
    }

    #[tokio::test]
    async fn tool_add_rule_system_and_repo() {
        let (repo, sid, root, _dir) = setup_session_with_root().await;
        let ctx = AgentContext { repo: &repo };
        // system rule
        let v = dispatch_tool(ctx, sid, "add_rule", serde_json::json!({"system": true, "name": "quality", "content": "Always lint."})).await.unwrap();
        assert!(v["summary"].as_str().unwrap().contains("system rule:quality"));
        let got = repo.get_rule("quality").await.unwrap().unwrap();
        assert_eq!(got.1, "Always lint.");

        // repo rule
        let v2 = dispatch_tool(AgentContext { repo: &repo }, sid, "add_rule", serde_json::json!({"name": "review-checklist", "content": "Look for tests.", "repo_dir": ".cursor/rules"})).await.unwrap();
        assert!(v2["summary"].as_str().unwrap().contains("repo rule:"));
        let rule_path = std::path::Path::new(&root).join(".cursor/rules/review-checklist.md");
        assert!(rule_path.exists());
    }

    #[tokio::test]
    async fn tool_files_write_move_delete_and_discovery_read() {
        let (repo, sid, root, _dir) = setup_session_with_root().await;
        let ctx = AgentContext { repo: &repo };
        // write
        std::fs::create_dir_all(std::path::Path::new(&root).join("dir")).unwrap();
        let _ = dispatch_tool(ctx, sid, "files.write", serde_json::json!({"path": "dir/x.txt", "content": "abc", "create": true, "dry_run": false, "preview_bytes": 16})).await.unwrap();
        // move
        let _ = dispatch_tool(AgentContext { repo: &repo }, sid, "files.move", serde_json::json!({"from": "dir/x.txt", "to": "dir/y.txt", "dry_run": false})).await.unwrap();
        assert!(std::path::Path::new(&root).join("dir/y.txt").exists());
        // discovery.read
        let v = dispatch_tool(AgentContext { repo: &repo }, sid, "discovery.read", serde_json::json!({"path": "dir/y.txt", "max_bytes": 64})).await.unwrap();
        assert_eq!(v["data"]["content"].as_str().unwrap(), "abc");
        // delete
        let _ = dispatch_tool(AgentContext { repo: &repo }, sid, "files.delete", serde_json::json!({"path": "dir/y.txt", "dry_run": false})).await.unwrap();
        assert!(!std::path::Path::new(&root).join("dir/y.txt").exists());
    }

    #[tokio::test]
    async fn tool_discovery_list_search() {
        let (repo, sid, root, _dir) = setup_session_with_root().await;
        fs::create_dir_all(std::path::Path::new(&root).join("src")).unwrap();
        fs::write(std::path::Path::new(&root).join("src/lib.rs"), b"mod x;").unwrap();
        let v = dispatch_tool(AgentContext { repo: &repo }, sid, "discovery.list", serde_json::json!({"max": 10})).await.unwrap();
        assert!(v["data"].is_array());
        let v2 = dispatch_tool(AgentContext { repo: &repo }, sid, "discovery.search", serde_json::json!({"pattern": "lib\\.rs$", "max": 10})).await.unwrap();
        assert!(v2["data"].as_array().unwrap().iter().any(|e| e["path"].as_str().unwrap().ends_with("lib.rs")));
    }

    #[tokio::test]
    async fn tool_include_url_forbidden_and_allowed() {
        use axum::{routing::get, Router};
        // setup session
        let (repo, sid, _root, _dir) = setup_session_with_root().await;
        // forbidden: no allowlist
        let ctx = AgentContext { repo: &repo };
        let err = dispatch_tool(ctx, sid, "include_url", serde_json::json!({"url": "http://127.0.0.1:9", "max_bytes": 64})).await.err();
        assert!(err.is_some());

        // allowed: start a tiny server and allow host
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/", get(|| async { "hello-url" }))).await.unwrap();
        });

        // update allowlist
        let mut sess = repo.get_session(sid).await.unwrap().unwrap();
        sess.settings.network_allowlist = Some(vec!["127.0.0.1".into()]);
        repo.update_settings(sid, sess.settings.clone()).await.unwrap();
        let url = format!("http://{}/", addr);
        let v = dispatch_tool(AgentContext { repo: &repo }, sid, "include_url", serde_json::json!({"url": url, "max_bytes": 64})).await.unwrap();
        assert!(v["summary"].as_str().unwrap().contains("url:"));
        // one context item should be stored
        use sqlx::Row;
        let row = sqlx::query("SELECT count(*) as c FROM context_items WHERE session_id = ?1 AND kind='url'")
            .bind(sid.to_string()).fetch_one(repo.pool()).await.unwrap();
        let c: i64 = row.get::<i64, _>("c");
        assert_eq!(c, 1);
    }

    #[tokio::test]
    async fn tool_git_status_add_commit_diff() {
        use git2::Repository;
        let (repo, sid, root, _dir) = setup_session_with_root().await;
        // init repo
        let _r = Repository::init(&root).unwrap();
        std::fs::write(std::path::Path::new(&root).join("a.txt"), b"content").unwrap();
        // status should see a.txt
        let st = dispatch_tool(AgentContext { repo: &repo }, sid, "git.status", serde_json::json!({})).await.unwrap();
        assert!(st["data"].as_array().unwrap().iter().any(|e| e["path"].as_str().unwrap().ends_with("a.txt")));
        // add and commit
        let _ = dispatch_tool(AgentContext { repo: &repo }, sid, "git.add_all", serde_json::json!({})).await.unwrap();
        let cm = dispatch_tool(AgentContext { repo: &repo }, sid, "git.commit", serde_json::json!({"message": "test"})).await.unwrap();
        assert!(cm["data"]["commit"].as_str().unwrap().len() > 5);
        // diff should be non-empty only if there are uncommitted changes
        let df = dispatch_tool(AgentContext { repo: &repo }, sid, "git.diff", serde_json::json!({})).await.unwrap();
        let diff_str = df["data"]["diff"].as_str().unwrap();
        assert!(diff_str.is_empty() || diff_str.contains("diff --git"));
    }
}

fn slugify(name: &str) -> String {
    let mut s = name.to_lowercase();
    s = s.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    while s.contains("--") { s = s.replace("--", "-"); }
    s.trim_matches('-').to_string()
}


