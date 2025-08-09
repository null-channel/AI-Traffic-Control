use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelParams {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ToolPolicies {
    pub dry_run: Option<bool>,
    pub max_read_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SessionSettings {
    pub default_model: Option<String>,
    pub model_params: Option<ModelParams>,
    pub project_root: Option<String>,
    pub tool_policies: Option<ToolPolicies>,
    pub network_allowlist: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GlobalConfigDefaults {
    pub default_model: Option<String>,
    pub model_params: Option<ModelParams>,
    pub tool_policies: Option<ToolPolicies>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RequestOverrides {
    pub model: Option<String>,
    pub model_params: Option<ModelParams>,
    pub tool_policies: Option<ToolPolicies>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffectiveSettings {
    pub model: Option<String>,
    pub model_params: ModelParams,
    pub project_root: Option<String>,
    pub tool_policies: ToolPolicies,
}

pub fn resolve_effective_settings(
    global: &GlobalConfigDefaults,
    session: &SessionSettings,
    request: &RequestOverrides,
) -> EffectiveSettings {
    let model = request
        .model
        .clone()
        .or_else(|| session.default_model.clone())
        .or_else(|| global.default_model.clone());

    let model_params = ModelParams {
        temperature: request
            .model_params
            .as_ref()
            .and_then(|p| p.temperature)
            .or_else(|| session.model_params.as_ref().and_then(|p| p.temperature))
            .or_else(|| global.model_params.as_ref().and_then(|p| p.temperature)),
        max_tokens: request
            .model_params
            .as_ref()
            .and_then(|p| p.max_tokens)
            .or_else(|| session.model_params.as_ref().and_then(|p| p.max_tokens))
            .or_else(|| global.model_params.as_ref().and_then(|p| p.max_tokens)),
        top_p: request
            .model_params
            .as_ref()
            .and_then(|p| p.top_p)
            .or_else(|| session.model_params.as_ref().and_then(|p| p.top_p))
            .or_else(|| global.model_params.as_ref().and_then(|p| p.top_p)),
    };

    let tool_policies = ToolPolicies {
        dry_run: request
            .tool_policies
            .as_ref()
            .and_then(|p| p.dry_run)
            .or_else(|| session.tool_policies.as_ref().and_then(|p| p.dry_run))
            .or_else(|| global.tool_policies.as_ref().and_then(|p| p.dry_run)),
        max_read_bytes: request
            .tool_policies
            .as_ref()
            .and_then(|p| p.max_read_bytes)
            .or_else(|| session
                .tool_policies
                .as_ref()
                .and_then(|p| p.max_read_bytes))
            .or_else(|| global
                .tool_policies
                .as_ref()
                .and_then(|p| p.max_read_bytes)),
    };

    EffectiveSettings {
        model,
        model_params,
        project_root: session.project_root.clone(),
        tool_policies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_request_over_session_over_global() {
        let global = GlobalConfigDefaults {
            default_model: Some("global-model".into()),
            model_params: Some(ModelParams {
                temperature: Some(0.1),
                max_tokens: Some(1000),
                top_p: Some(0.9),
            }),
            tool_policies: Some(ToolPolicies {
                dry_run: Some(true),
                max_read_bytes: Some(1024),
            }),
        };

        let session = SessionSettings {
            default_model: Some("session-model".into()),
            model_params: Some(ModelParams {
                temperature: Some(0.2),
                max_tokens: None,
                top_p: None,
            }),
            project_root: Some("/repo".into()),
            tool_policies: Some(ToolPolicies {
                dry_run: Some(false),
                max_read_bytes: None,
            }),
            network_allowlist: None,
        };

        let request = RequestOverrides {
            model: Some("request-model".into()),
            model_params: Some(ModelParams {
                temperature: None,
                max_tokens: Some(2048),
                top_p: None,
            }),
            tool_policies: Some(ToolPolicies {
                dry_run: None,
                max_read_bytes: Some(2048),
            }),
        };

        let eff = resolve_effective_settings(&global, &session, &request);

        assert_eq!(eff.model.as_deref(), Some("request-model"));
        assert_eq!(eff.model_params.temperature, Some(0.2)); // from session
        assert_eq!(eff.model_params.max_tokens, Some(2048)); // from request
        assert_eq!(eff.model_params.top_p, Some(0.9)); // from global
        assert_eq!(eff.project_root.as_deref(), Some("/repo"));
        assert_eq!(eff.tool_policies.dry_run, Some(false)); // from session
        assert_eq!(eff.tool_policies.max_read_bytes, Some(2048)); // from request
    }
}


