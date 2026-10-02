//! テスト用のモックプロバイダ

use ai_provider::provider::AiProvider;
use ai_provider::types::{GenerateRequest, GenerateResponse, ProviderError};

/// 最後のメッセージをそのまま返すプロバイダ。名前とモデルを差し替えて照合を試せる
pub(crate) struct MockProvider {
    pub(crate) name: &'static str,
    pub(crate) model: Option<&'static str>,
    /// true なら `generate()` が常に失敗する
    pub(crate) fails: bool,
}

impl MockProvider {
    pub(crate) const fn new(name: &'static str, model: Option<&'static str>) -> Self {
        Self {
            name,
            model,
            fails: false,
        }
    }

    /// 既定と同じ名前・モデルで、生成だけが失敗するプロバイダ
    pub(crate) const fn failing() -> Self {
        Self {
            fails: true,
            ..Self::new("mock", None)
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new("mock", None)
    }
}

impl AiProvider for MockProvider {
    fn name(&self) -> &str {
        self.name
    }

    fn model(&self) -> Option<&str> {
        self.model
    }

    fn is_available(&self) -> bool {
        true
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        if self.fails {
            return Err(ProviderError::GenerationFailed("mock failure".to_string()));
        }
        let last = request.messages.last().map_or("", |m| m.content.as_str());
        Ok(GenerateResponse {
            content: format!("echo: {last}"),
            provider: self.name.to_string(),
        })
    }
}
