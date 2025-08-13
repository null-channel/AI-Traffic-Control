use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct ListTool;
pub struct SearchTool;
pub struct ReadTool;

impl Tool for ListTool {
    fn name(&self) -> &'static str { "discovery.list" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let max = args.get("max").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let items = crate::discovery::list_files(&root, max);
            Ok(ToolResult { summary: format!("{} items", items.len()), data: Some(serde_json::to_value(items)?) })
        })
    }
}

impl Tool for SearchTool {
    fn name(&self) -> &'static str { "discovery.search" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let max = args.get("max").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing pattern"))?;
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let items = crate::discovery::search_files(&root, pattern, max);
            Ok(ToolResult { summary: format!("{} matches", items.len()), data: Some(serde_json::to_value(items)?) })
        })
    }
}

impl Tool for ReadTool {
    fn name(&self) -> &'static str { "discovery.read" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let max_bytes = args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(65536) as usize;
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let content = crate::discovery::read_file_under_root(&root, path, max_bytes)?;
            Ok(ToolResult { summary: format!("read:{} bytes:{}", path, content.len()), data: Some(serde_json::json!({"path": path, "content": content})) })
        })
    }
}


