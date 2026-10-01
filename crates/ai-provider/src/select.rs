//! CLI からのプロバイダ選択
//!
//! フラグ・環境変数・再開するセッションの記録値の優先順位をここに集め、
//! CLI の外でもテストできるようにする。

use crate::lmstudio::{DEFAULT_MODEL, LmStudioConfig, LmStudioProvider};
use crate::provider::AiProvider;
use crate::types::ProviderError;
use crate::{APPLE_INTELLIGENCE, AppleIntelligenceProvider, LM_STUDIO};

/// 選択できるプロバイダ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Apple,
    LmStudio,
}

impl ProviderKind {
    /// `--provider` / `RINTEL_PROVIDER` の値から解釈する
    pub fn from_value(value: &str) -> Result<Self, ProviderError> {
        match value {
            "apple" => Ok(Self::Apple),
            "lm-studio" => Ok(Self::LmStudio),
            _ => Err(ProviderError::Other(format!(
                "unknown provider '{value}' (expected apple or lm-studio)"
            ))),
        }
    }

    /// セッションに記録された名前（`AiProvider::name()`）から解釈する
    fn from_name(name: &str) -> Option<Self> {
        match name {
            APPLE_INTELLIGENCE => Some(Self::Apple),
            LM_STUDIO => Some(Self::LmStudio),
            _ => None,
        }
    }
}

/// 解決済みのプロバイダ指定
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSpec {
    Apple,
    LmStudio { model: String },
}

impl ProviderSpec {
    /// フラグ・環境変数・セッションの記録値からプロバイダを決める
    ///
    /// - 新規: `--provider` → `RINTEL_PROVIDER` → apple。モデルは `--model` → `RINTEL_LMS_MODEL` → 既定
    /// - 再開（`recorded` あり）: 記録値を使い、環境変数は見ない。明示されたフラグが記録と
    ///   異なればエラーにし、別のプロバイダ・モデルで会話を続けない
    ///
    /// `recorded` は再開するセッションの (プロバイダ名, モデル)。
    pub fn resolve(
        explicit_provider: Option<ProviderKind>,
        explicit_model: Option<&str>,
        recorded: Option<(&str, Option<&str>)>,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ProviderError> {
        if explicit_model.is_some_and(str::is_empty) {
            return Err(ProviderError::Other(
                "--model must not be empty".to_string(),
            ));
        }

        let spec = match recorded {
            Some((name, model)) => Self::from_record(name, model)?,
            None => Self::from_flags_or_env(explicit_provider, explicit_model, &lookup)?,
        };
        // 再開時は記録との不一致を先に伝える。Apple のセッションに `--provider lm-studio --model x`
        // を付けたとき、下の `--model` の検査が先だと lm-studio を指定済みなのに指定を促してしまう
        if let Some((name, model)) = recorded {
            let is_provider_mismatch = explicit_provider.is_some_and(|kind| kind != spec.kind());
            let is_model_mismatch = explicit_model.is_some_and(|m| Some(m) != spec.model());
            if is_provider_mismatch || is_model_mismatch {
                return Err(ProviderError::Other(format!(
                    "the session was created with {}; omit --provider and --model to resume it",
                    describe_provider(name, model)
                )));
            }
        }

        if spec == Self::Apple && explicit_model.is_some() {
            return Err(ProviderError::Other(
                "--model is only supported with --provider lm-studio".to_string(),
            ));
        }
        Ok(spec)
    }

    /// プロバイダを組み立てる。LM Studio の接続設定は `lookup`（通常は環境変数）から読む
    pub fn build(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Box<dyn AiProvider>, ProviderError> {
        match self {
            Self::Apple => Ok(Box::new(AppleIntelligenceProvider::new())),
            Self::LmStudio { model } => {
                let config = LmStudioConfig::from_lookup(model.clone(), lookup)?;
                Ok(Box::new(LmStudioProvider::new(config)))
            }
        }
    }

    #[must_use]
    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Apple => ProviderKind::Apple,
            Self::LmStudio { .. } => ProviderKind::LmStudio,
        }
    }

    #[must_use]
    pub fn model(&self) -> Option<&str> {
        match self {
            Self::Apple => None,
            Self::LmStudio { model } => Some(model),
        }
    }

    fn from_record(name: &str, model: Option<&str>) -> Result<Self, ProviderError> {
        match (ProviderKind::from_name(name), model) {
            (Some(ProviderKind::Apple), None) => Ok(Self::Apple),
            (Some(ProviderKind::LmStudio), Some(model)) => Ok(Self::LmStudio {
                model: model.to_string(),
            }),
            _ => Err(ProviderError::Other(format!(
                "cannot resume a session created with {}",
                describe_provider(name, model)
            ))),
        }
    }

    fn from_flags_or_env(
        explicit_provider: Option<ProviderKind>,
        explicit_model: Option<&str>,
        lookup: &impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ProviderError> {
        let get = |key: &str| lookup(key).filter(|value| !value.is_empty());

        let kind = match explicit_provider {
            Some(kind) => kind,
            None => match get("RINTEL_PROVIDER") {
                Some(value) => ProviderKind::from_value(&value)?,
                None => ProviderKind::Apple,
            },
        };
        Ok(match kind {
            ProviderKind::Apple => Self::Apple,
            ProviderKind::LmStudio => Self::LmStudio {
                model: explicit_model
                    .map(String::from)
                    .or_else(|| get("RINTEL_LMS_MODEL"))
                    .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            },
        })
    }
}

/// 表示用のプロバイダ名（例: `lm-studio (qwen/qwen3.6-35b-a3b)`）
#[must_use]
pub fn describe_provider(name: &str, model: Option<&str>) -> String {
    match model {
        Some(model) => format!("{name} ({model})"),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    fn lm_studio(model: &str) -> ProviderSpec {
        ProviderSpec::LmStudio {
            model: model.to_string(),
        }
    }

    #[test]
    fn defaults_to_apple() {
        let spec = ProviderSpec::resolve(None, None, None, env(&[])).unwrap();
        assert_eq!(spec, ProviderSpec::Apple);
    }

    #[test]
    fn env_selects_lm_studio_with_default_model() {
        let spec =
            ProviderSpec::resolve(None, None, None, env(&[("RINTEL_PROVIDER", "lm-studio")]))
                .unwrap();
        assert_eq!(spec, lm_studio(DEFAULT_MODEL));
    }

    #[test]
    fn env_model_is_used() {
        let lookup = env(&[
            ("RINTEL_PROVIDER", "lm-studio"),
            ("RINTEL_LMS_MODEL", "env/model"),
        ]);
        let spec = ProviderSpec::resolve(None, None, None, lookup).unwrap();
        assert_eq!(spec, lm_studio("env/model"));
    }

    #[test]
    fn flags_override_env() {
        let lookup = env(&[
            ("RINTEL_PROVIDER", "lm-studio"),
            ("RINTEL_LMS_MODEL", "env/model"),
        ]);
        let spec = ProviderSpec::resolve(Some(ProviderKind::Apple), None, None, &lookup).unwrap();
        assert_eq!(spec, ProviderSpec::Apple);

        let spec = ProviderSpec::resolve(None, Some("flag/model"), None, &lookup).unwrap();
        assert_eq!(spec, lm_studio("flag/model"));

        let spec = ProviderSpec::resolve(
            Some(ProviderKind::LmStudio),
            None,
            None,
            env(&[("RINTEL_PROVIDER", "apple")]),
        )
        .unwrap();
        assert_eq!(spec, lm_studio(DEFAULT_MODEL));
    }

    #[test]
    fn empty_env_values_are_unset() {
        let lookup = env(&[("RINTEL_PROVIDER", ""), ("RINTEL_LMS_MODEL", "")]);
        let spec = ProviderSpec::resolve(None, None, None, &lookup).unwrap();
        assert_eq!(spec, ProviderSpec::Apple);

        let spec =
            ProviderSpec::resolve(Some(ProviderKind::LmStudio), None, None, &lookup).unwrap();
        assert_eq!(spec, lm_studio(DEFAULT_MODEL));
    }

    #[test]
    fn model_requires_lm_studio() {
        let error = ProviderSpec::resolve(Some(ProviderKind::Apple), Some("x"), None, env(&[]))
            .unwrap_err();
        assert!(error.to_string().contains("--model"));

        let error = ProviderSpec::resolve(None, Some("x"), None, env(&[])).unwrap_err();
        assert!(error.to_string().contains("--model"));
    }

    #[test]
    fn rejects_empty_model_flag() {
        let error = ProviderSpec::resolve(Some(ProviderKind::LmStudio), Some(""), None, env(&[]))
            .unwrap_err();
        assert!(error.to_string().contains("--model"));
    }

    #[test]
    fn rejects_unknown_env_provider() {
        let error = ProviderSpec::resolve(None, None, None, env(&[("RINTEL_PROVIDER", "typo")]))
            .unwrap_err();
        assert!(error.to_string().contains("typo"));
    }

    #[test]
    fn resume_uses_record_and_ignores_env() {
        let lookup = env(&[
            ("RINTEL_PROVIDER", "apple"),
            ("RINTEL_LMS_MODEL", "env/model"),
        ]);
        let spec = ProviderSpec::resolve(None, None, Some((LM_STUDIO, Some("rec/model"))), &lookup)
            .unwrap();
        assert_eq!(spec, lm_studio("rec/model"));

        let spec = ProviderSpec::resolve(
            None,
            None,
            Some((APPLE_INTELLIGENCE, None)),
            env(&[("RINTEL_PROVIDER", "lm-studio")]),
        )
        .unwrap();
        assert_eq!(spec, ProviderSpec::Apple);
    }

    #[test]
    fn resume_accepts_matching_flags() {
        let spec = ProviderSpec::resolve(
            Some(ProviderKind::LmStudio),
            Some("rec/model"),
            Some((LM_STUDIO, Some("rec/model"))),
            env(&[]),
        )
        .unwrap();
        assert_eq!(spec, lm_studio("rec/model"));
    }

    #[test]
    fn resume_rejects_conflicting_flags() {
        let recorded = Some((LM_STUDIO, Some("rec/model")));
        let error =
            ProviderSpec::resolve(None, Some("other/model"), recorded, env(&[])).unwrap_err();
        assert!(error.to_string().contains("rec/model"));

        let error =
            ProviderSpec::resolve(Some(ProviderKind::Apple), None, recorded, env(&[])).unwrap_err();
        assert!(error.to_string().contains("lm-studio"));

        let error = ProviderSpec::resolve(
            Some(ProviderKind::LmStudio),
            None,
            Some((APPLE_INTELLIGENCE, None)),
            env(&[]),
        )
        .unwrap_err();
        assert!(error.to_string().contains(APPLE_INTELLIGENCE));

        for provider in [None, Some(ProviderKind::LmStudio)] {
            let error = ProviderSpec::resolve(
                provider,
                Some("other/model"),
                Some((APPLE_INTELLIGENCE, None)),
                env(&[]),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("created with apple-intelligence"),
                "{provider:?}: {error}"
            );
        }
    }

    #[test]
    fn resume_rejects_unknown_or_inconsistent_record() {
        for recorded in [
            ("ollama", None),
            (LM_STUDIO, None),
            (APPLE_INTELLIGENCE, Some("model")),
        ] {
            let result = ProviderSpec::resolve(None, None, Some(recorded), env(&[]));
            assert!(result.is_err(), "{recorded:?}");
        }
    }

    #[test]
    fn build_validates_lm_studio_config() {
        let error = lm_studio("m")
            .build(env(&[("RINTEL_LMS_TIMEOUT", "abc")]))
            .err()
            .unwrap();
        assert!(error.to_string().contains("RINTEL_LMS_TIMEOUT"));

        let provider = lm_studio("m").build(env(&[])).unwrap();
        assert_eq!(provider.name(), LM_STUDIO);
        assert_eq!(provider.model(), Some("m"));
    }

    #[test]
    fn build_apple_ignores_lm_studio_env() {
        let provider = ProviderSpec::Apple
            .build(env(&[("RINTEL_LMS_TIMEOUT", "abc")]))
            .unwrap();
        assert_eq!(provider.name(), APPLE_INTELLIGENCE);
        assert_eq!(provider.model(), None);
    }

    #[test]
    fn describes_provider() {
        assert_eq!(describe_provider(LM_STUDIO, Some("m")), "lm-studio (m)");
        assert_eq!(
            describe_provider(APPLE_INTELLIGENCE, None),
            "apple-intelligence"
        );
    }
}
