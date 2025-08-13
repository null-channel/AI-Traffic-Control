use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct StatusTool;
pub struct DiffTool;
pub struct AddAllTool;
pub struct CommitTool;

impl Tool for StatusTool {
    fn name(&self) -> &'static str { "git.status" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, _args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let st = crate::git_ops::status(&root)?;
            Ok(ToolResult { summary: format!("{} entries", st.len()), data: Some(serde_json::to_value(st)?) })
        })
    }
}

impl Tool for DiffTool {
    fn name(&self) -> &'static str { "git.diff" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, _args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let d = crate::git_ops::diff_porcelain(&root)?;
            Ok(ToolResult { summary: format!("{} chars", d.len()), data: Some(serde_json::json!({"diff": d})) })
        })
    }
}

impl Tool for AddAllTool {
    fn name(&self) -> &'static str { "git.add_all" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, _args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            crate::git_ops::add_all(&root)?;
            Ok(ToolResult { summary: "git add -A".into(), data: Some(serde_json::json!({"ok": true})) })
        })
    }
}

impl Tool for CommitTool {
    fn name(&self) -> &'static str { "git.commit" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let message = args.get("message").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing message"))?;
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let oid = crate::git_ops::commit(&root, message)?;
            Ok(ToolResult { summary: format!("commit:{}", oid), data: Some(serde_json::json!({"commit": oid})) })
        })
    }
}


