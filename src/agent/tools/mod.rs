use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::settings::SessionSettings;
use crate::storage::SessionRepository;

pub mod include_file;
pub mod include_url;
pub mod rules;
pub mod discovery_tools;
pub mod file_tools;
pub mod git_tools;

pub struct ToolContext<'a> {
    pub repo: &'a dyn SessionRepository,
    pub session_id: Uuid,
    pub settings: &'a SessionSettings,
}

pub struct ToolResult {
    pub summary: String,
    pub data: Option<Value>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn run<'a>(&'a self, ctx: ToolContext<'a>, args: Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolResult>> + Send + 'a>>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>, 
}

impl ToolRegistry {
    pub fn new() -> Self { Self { tools: Vec::new() } }
    pub fn with_default_tools() -> Self {
        let mut r = Self::new();
        r.register(Box::new(include_file::IncludeFileTool));
        r.register(Box::new(include_url::IncludeUrlTool));
        r.register(Box::new(rules::AddRuleTool));
        r.register(Box::new(discovery_tools::ListTool));
        r.register(Box::new(discovery_tools::SearchTool));
        r.register(Box::new(discovery_tools::ReadTool));
        r.register(Box::new(file_tools::WriteTool));
        r.register(Box::new(file_tools::MoveTool));
        r.register(Box::new(file_tools::DeleteTool));
        r.register(Box::new(git_tools::StatusTool));
        r.register(Box::new(git_tools::DiffTool));
        r.register(Box::new(git_tools::AddAllTool));
        r.register(Box::new(git_tools::CommitTool));
        r
    }
    pub fn register(&mut self, t: Box<dyn Tool>) { self.tools.push(t); }
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().map(|b| b.as_ref()).find(|t| t.name() == name)
    }
}


