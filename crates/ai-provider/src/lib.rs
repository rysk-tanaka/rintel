pub mod lmstudio;
mod prompt;
pub mod provider;
pub mod select;
pub mod types;

/// Apple Intelligence のプロバイダ名（`AiProvider::name()`、セッションに記録される）
pub const APPLE_INTELLIGENCE: &str = "apple-intelligence";

/// LM Studio のプロバイダ名（`AiProvider::name()`、セッションに記録される）
pub const LM_STUDIO: &str = "lm-studio";

#[cfg(target_os = "macos")]
mod apple;
#[cfg(target_os = "macos")]
pub use apple::AppleIntelligenceProvider;

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
pub use stub::AppleIntelligenceProvider;

pub use lmstudio::{LmStudioConfig, LmStudioProvider};
pub use select::{ProviderKind, ProviderSpec, describe_provider};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::AiProvider;

    #[test]
    fn provider_does_not_panic() {
        let provider = AppleIntelligenceProvider::new();
        let _ = provider.is_available();
    }
}
