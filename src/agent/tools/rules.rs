use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;

pub struct AddRuleTool;

impl Tool for AddRuleTool {
    fn name(&self) -> &'static str { "add_rule" }

    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing name"))?;
            let content = args.get("content").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("missing content"))?;
            let system = args.get("system").and_then(|v| v.as_bool()).unwrap_or(false);
            if system {
                ctx.repo.upsert_rule(name, content).await?;
                return Ok(ToolResult { summary: format!("system rule:{}", name), data: None });
            }
            let repo_dir = args.get("repo_dir").and_then(|v| v.as_str()).unwrap_or(".cursor/rules");
            let root = ctx.settings.project_root.clone().ok_or_else(|| anyhow::anyhow!("no project_root"))?;
            let path = std::path::Path::new(&root).join(repo_dir).join(format!("{}.md", slugify(name)));
            std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new(&root)))?;
            std::fs::write(&path, content.as_bytes())?;
            Ok(ToolResult { summary: format!("repo rule:{}", path.display()), data: None })
        })
    }
}

fn slugify(name: &str) -> String {
    let mut s = name.to_lowercase();
    s = s.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    while s.contains("--") { s = s.replace("--", "-"); }
    s.trim_matches('-').to_string()
}


