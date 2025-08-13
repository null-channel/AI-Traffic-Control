use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct IncludeUrlTool;

impl Tool for IncludeUrlTool {
    fn name(&self) -> &'static str { "include_url" }

    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let url = args.get("url").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing url"))?;
            let max_bytes = args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(262144) as usize;
            let parsed = url::Url::parse(url)?;
            let host = parsed.host_str().ok_or_else(|| anyhow::anyhow!("invalid host"))?;
            if !crate::server::is_allowed_host(&ctx.settings.network_allowlist, host) { anyhow::bail!("host not allowlisted"); }
            let content = crate::server::fetch_and_extract(url, max_bytes).await?;
            ctx.repo.add_context_item(ctx.session_id, "url", url, &content, content.len() as i64).await?;
            Ok(ToolResult { summary: format!("url:{} bytes:{}", url, content.len()), data: Some(serde_json::json!({"url": url, "bytes": content.len()})) })
        })
    }
}


