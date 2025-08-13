use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct WriteTool;
pub struct MoveTool;
pub struct DeleteTool;

impl Tool for WriteTool {
    fn name(&self) -> &'static str { "files.write" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let content = args.get("content").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing content"))?;
            let create = args.get("create").and_then(|v| v.as_bool()).unwrap_or(true);
            let preview_bytes = args.get("preview_bytes").and_then(|v| v.as_u64()).unwrap_or(1024) as usize;
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or_else(|| ctx.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let res = crate::file_ops::write_file_under_root(&root, path, content, create, dry_run, preview_bytes)?;
            Ok(ToolResult { summary: format!("write:{} applied:{}", path, res.applied), data: Some(serde_json::to_value(res)?) })
        })
    }
}

impl Tool for MoveTool {
    fn name(&self) -> &'static str { "files.move" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let from = args.get("from").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let to = args.get("to").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or_else(|| ctx.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let res = crate::file_ops::move_file_under_root(&root, from, to, dry_run)?;
            Ok(ToolResult { summary: format!("move:{} -> {} applied:{}", from, to, res.applied), data: Some(serde_json::to_value(res)?) })
        })
    }
}

impl Tool for DeleteTool {
    fn name(&self) -> &'static str { "files.delete" }
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing path"))?;
            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or_else(|| ctx.settings.tool_policies.as_ref().and_then(|p| p.dry_run).unwrap_or(true));
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let res = crate::file_ops::delete_file_under_root(&root, path, dry_run)?;
            Ok(ToolResult { summary: format!("delete:{} applied:{}", path, res.applied), data: Some(serde_json::to_value(res)?) })
        })
    }
}


