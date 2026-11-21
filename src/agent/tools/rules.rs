use super::{Tool, ToolContext, ToolResult};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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


pub struct GetEffectiveRulesTool;

impl Tool for GetEffectiveRulesTool {
    fn name(&self) -> &'static str { "rules.get_effective" }

    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>> {
        Box::pin(async move {
            // Load system rules
            let system_rules = ctx.repo.list_rules().await?; // Vec<(name, content)>
            let mut merged: HashMap<String, (String, String, &'static str)> = HashMap::new(); // slug -> (name, content, source)
            for (name, content) in system_rules.iter() {
                merged.insert(slugify(name), (name.clone(), content.clone(), "system"));
            }

            // Load repo rules if project_root present
            let mut overrides = 0usize;
            if let Some(root) = ctx.settings.project_root.clone() {
                let repo_dir = args.get("repo_dir").and_then(|v| v.as_str()).unwrap_or(".cursor/rules");
                let dir = Path::new(&root).join(repo_dir);
                if dir.exists() && dir.is_dir() {
                    for entry in fs::read_dir(&dir)? {
                        let entry = entry?;
                        let path = entry.path();
                        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md") {
                            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                            let content = fs::read_to_string(&path)?;
                            let key = stem.clone(); // filename stem is the slug
                            let name = merged.get(&key).map(|(n, _, _)| n.clone()).unwrap_or_else(|| stem.replace('-', " "));
                            if merged.contains_key(&key) { overrides += 1; }
                            merged.insert(key, (name, content, "repo"));
                        }
                    }
                }
            }

            // Build ordered list (sort by name)
            let mut items: Vec<(String, String, &'static str)> = merged.into_iter().map(|(_, v)| v).collect();
            items.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

            let data_rules: Vec<serde_json::Value> = items.into_iter().map(|(name, content, source)| {
                serde_json::json!({ "name": name, "source": source, "content": content })
            }).collect();

            Ok(ToolResult {
                summary: format!("{} rules (repo overrides {})", data_rules.len(), overrides),
                data: Some(serde_json::json!({ "rules": data_rules })),
            })
        })
    }
}


