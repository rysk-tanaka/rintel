use std::path::Path;

use ai_provider::provider::AiProvider;
use ai_provider::types::{FileContext, GenerateRequest, Message, Role};
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// AI プロバイダとの会話セッション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    /// TTL in seconds (None = no expiration)
    pub ttl_secs: Option<u64>,
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    pub file_contexts: Vec<FileContext>,
}

impl Session {
    /// 新しいセッションを作成する
    #[must_use]
    pub fn new(system_prompt: Option<String>, ttl_secs: Option<u64>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: None,
            created_at: now,
            last_active: now,
            ttl_secs,
            system_prompt,
            messages: Vec::new(),
            file_contexts: Vec::new(),
        }
    }

    /// セッションが期限切れかどうか
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let Some(ttl_secs) = self.ttl_secs else {
            return false;
        };
        let elapsed = Utc::now()
            .signed_duration_since(self.last_active)
            .num_seconds();
        elapsed > 0 && elapsed as u64 > ttl_secs
    }

    /// ファイルをコンテキストに追加する
    pub fn add_file_context(&mut self, path: &Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let filename = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().to_string());
        self.file_contexts.push(FileContext { filename, content });
        Ok(())
    }

    /// ユーザー入力を送信し、AI の応答を返す
    pub fn send(
        &mut self,
        provider: &dyn AiProvider,
        input: &str,
    ) -> Result<String, ai_provider::types::ProviderError> {
        self.messages.push(Message {
            role: Role::User,
            content: input.to_string(),
            timestamp: Utc::now(),
        });

        let request = GenerateRequest {
            system_prompt: self.system_prompt.clone(),
            messages: self.messages.clone(),
            file_contexts: self.file_contexts.clone(),
        };

        let response = provider.generate(&request)?;

        self.messages.push(Message {
            role: Role::Assistant,
            content: response.content.clone(),
            timestamp: Utc::now(),
        });

        self.last_active = Utc::now();

        Ok(response.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_provider::types::{GenerateResponse, ProviderError};

    struct MockProvider;

    impl AiProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn is_available(&self) -> bool {
            true
        }

        fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
            let last = request
                .messages
                .last()
                .map_or("", |m| m.content.as_str());
            Ok(GenerateResponse {
                content: format!("echo: {last}"),
                provider: "mock".to_string(),
            })
        }
    }

    #[test]
    fn send_appends_messages() {
        let mut session = Session::new(None, None);
        let provider = MockProvider;

        let reply = session.send(&provider, "hello").unwrap();
        assert_eq!(reply, "echo: hello");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[1].role, Role::Assistant);
    }

    #[test]
    fn send_preserves_history() {
        let mut session = Session::new(Some("system".to_string()), None);
        let provider = MockProvider;

        session.send(&provider, "first").unwrap();
        session.send(&provider, "second").unwrap();

        assert_eq!(session.messages.len(), 4);
        assert_eq!(session.messages[2].content, "second");
    }

    #[test]
    fn is_expired_without_ttl() {
        let session = Session::new(None, None);
        assert!(!session.is_expired());
    }

    #[test]
    fn is_expired_with_future_ttl() {
        let session = Session::new(None, Some(3600));
        assert!(!session.is_expired());
    }

    #[test]
    fn is_expired_with_past_ttl() {
        let mut session = Session::new(None, Some(0));
        // Force last_active to the past
        session.last_active = Utc::now() - chrono::Duration::seconds(10);
        assert!(session.is_expired());
    }
}
