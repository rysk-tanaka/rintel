pub mod provider;
pub mod types;

#[cfg(target_os = "macos")]
mod apple;
#[cfg(target_os = "macos")]
pub use apple::AppleIntelligenceProvider;

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
pub use stub::AppleIntelligenceProvider;

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
