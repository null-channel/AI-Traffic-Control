use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelParams {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ModelParamsPatch {
    pub temperature: Option<Option<f32>>, // Some(None) clears, Some(Some(v)) sets
    pub max_tokens: Option<Option<u32>>,
    pub top_p: Option<Option<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ToolPolicies {
    pub dry_run: Option<bool>,
    pub max_read_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ToolPoliciesPatch {
    pub dry_run: Option<Option<bool>>,
    pub max_read_bytes: Option<Option<u64>>,
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
pub struct SessionSettingsPatch {
    pub default_model: Option<Option<String>>,
    pub model_params: Option<ModelParamsPatch>,
    pub project_root: Option<Option<String>>,
    pub tool_policies: Option<ToolPoliciesPatch>,
    pub network_allowlist: Option<Option<Vec<String>>>,
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

impl SessionSettings {
    pub fn apply_patch(&mut self, patch: SessionSettingsPatch) {
        if let Some(dm) = patch.default_model {
            self.default_model = dm;
        }
        if let Some(mp) = patch.model_params {
            let mut current = self.model_params.clone().unwrap_or_default();
            if let Some(t) = mp.temperature { current.temperature = t; }
            if let Some(m) = mp.max_tokens { current.max_tokens = m; }
            if let Some(p) = mp.top_p { current.top_p = p; }
            self.model_params = Some(current);
        }
        if let Some(pr) = patch.project_root {
            self.project_root = pr;
        }
        if let Some(tp) = patch.tool_policies {
            let mut current = self.tool_policies.clone().unwrap_or_default();
            if let Some(d) = tp.dry_run { current.dry_run = d; }
            if let Some(m) = tp.max_read_bytes { current.max_read_bytes = m; }
            self.tool_policies = Some(current);
        }
        if let Some(na) = patch.network_allowlist {
            self.network_allowlist = na;
        }
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

    #[test]
    fn patch_updates_nested_fields_and_allows_clear() {
        let mut session = SessionSettings {
            default_model: Some("gpt-4".into()),
            model_params: Some(ModelParams { temperature: Some(0.5), max_tokens: Some(1024), top_p: Some(1.0) }),
            project_root: Some("/repo".into()),
            tool_policies: Some(ToolPolicies { dry_run: Some(true), max_read_bytes: Some(1024) }),
            network_allowlist: Some(vec!["example.com".into()]),
        };

        let patch = SessionSettingsPatch {
            default_model: Some(Some("gpt-4o".into())),
            model_params: Some(ModelParamsPatch { temperature: Some(Some(0.2)), max_tokens: Some(None), top_p: None }),
            project_root: Some(None),
            tool_policies: Some(ToolPoliciesPatch { dry_run: Some(Some(false)), max_read_bytes: Some(Some(2048)) }),
            network_allowlist: Some(Some(vec!["docs.rs".into()])),
        };

        session.apply_patch(patch);

        assert_eq!(session.default_model.as_deref(), Some("gpt-4o"));
        let mp = session.model_params.unwrap();
        assert_eq!(mp.temperature, Some(0.2));
        assert_eq!(mp.max_tokens, None); // cleared
        assert_eq!(mp.top_p, Some(1.0)); // unchanged
        assert_eq!(session.project_root, None); // cleared
        let tp = session.tool_policies.unwrap();
        assert_eq!(tp.dry_run, Some(false));
        assert_eq!(tp.max_read_bytes, Some(2048));
        assert_eq!(session.network_allowlist, Some(vec!["docs.rs".into()]));
    }
}


