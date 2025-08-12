use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelRequest {
    pub model: String,
    pub prompt: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelResponse {
    pub content: String,
    pub model: String,
}

#[async_trait]
pub trait LanguageModel: Send + Sync {
    async fn generate(&self, req: ModelRequest) -> anyhow::Result<ModelResponse>;
}

#[derive(Clone)]
pub struct OpenAICompatible {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl OpenAICompatible {
    pub fn from_env() -> Self {
        let base_url = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into());
        let api_key = std::env::var("OPENAI_API_KEY").ok();
        Self { base_url, api_key }
    }
}

impl Default for OpenAICompatible {
    fn default() -> Self { Self::from_env() }
}

#[derive(Debug, Serialize)]
struct OaiChatRequest<'a> {
    model: &'a str,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")] temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")] max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")] top_p: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct OaiChatResponse {
    choices: Vec<OaiChoice>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OaiChoice { message: OaiMessage }

#[derive(Debug, Deserialize)]
struct OaiMessage { content: String }

#[async_trait]
impl LanguageModel for OpenAICompatible {
    async fn generate(&self, req: ModelRequest) -> anyhow::Result<ModelResponse> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = OaiChatRequest {
            model: &req.model,
            messages: vec![serde_json::json!({"role":"user","content": req.prompt})],
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            top_p: req.top_p,
        };
        let client = reqwest::Client::new();
        let mut rb = client.post(url).json(&body);
        if let Some(key) = &self.api_key {
            rb = rb.bearer_auth(key);
        }
        let resp = rb.send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("model call failed: {}", resp.status());
        }
        let v: OaiChatResponse = resp.json().await?;
        let content = v.choices.get(0).map(|c| c.message.content.clone()).unwrap_or_default();
        Ok(ModelResponse { content, model: v.model })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelSelector;

impl ModelSelector {
    pub fn select(model_override: Option<String>, session_default: Option<String>, global_default: Option<String>) -> Option<String> {
        model_override.or(session_default).or(global_default)
    }
}


