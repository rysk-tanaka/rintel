use crate::provider::AiProvider;
use crate::types::{GenerateRequest, GenerateResponse, ProviderError};

/// 非 macOS 環境用のスタブプロバイダ
#[derive(Clone)]
pub struct AppleIntelligenceProvider;

impl AppleIntelligenceProvider {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for AppleIntelligenceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AiProvider for AppleIntelligenceProvider {
    fn name(&self) -> &str {
        crate::APPLE_INTELLIGENCE
    }

    fn is_available(&self) -> bool {
        false
    }

    fn unavailable_message(&self) -> String {
        "Apple Intelligence is not available on this system.".to_string()
    }

    fn generate(&self, _request: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        Err(ProviderError::NotAvailable)
    }
}
