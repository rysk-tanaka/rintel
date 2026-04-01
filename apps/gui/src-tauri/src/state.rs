use std::sync::Mutex;

use ai_provider::AppleIntelligenceProvider;
use ai_session::{SessionConfig, SessionManager};

pub struct AppState {
    pub provider: AppleIntelligenceProvider,
    pub session_manager: Mutex<SessionManager>,
}

impl AppState {
    #[allow(clippy::expect_used)]
    pub fn new() -> Self {
        let config = SessionConfig::default();
        let manager =
            SessionManager::new(&config.storage_dir).expect("failed to initialize session manager");

        Self {
            provider: AppleIntelligenceProvider::new(),
            session_manager: Mutex::new(manager),
        }
    }
}
