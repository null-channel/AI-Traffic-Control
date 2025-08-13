use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct IncludeFileTool;

impl Tool for IncludeFileTool {
    fn name(&self) -> &'static str { "include_file" }

    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let max_bytes = args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(65536) as usize;
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let content = crate::discovery::read_file_under_root(&root, path, max_bytes)?;
            ctx.repo.add_context_item(ctx.session_id, "file", path, &content, content.len() as i64).await?;
            Ok(ToolResult { summary: format!("file:{} bytes:{}", path, content.len()), data: Some(serde_json::json!({"path": path, "bytes": content.len()})) })
        })
    }
}


