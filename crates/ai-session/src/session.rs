use std::path::Path;

use ai_provider::provider::AiProvider;
use ai_provider::types::{FileContext, GenerateRequest, Message, ProviderError, Role};
use ai_provider::{APPLE_INTELLIGENCE, describe_provider};
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
    /// 会話に使うプロバイダ名。None は記録を始める前のセッションで、Apple Intelligence として扱う
    #[serde(default)]
    pub provider: Option<String>,
    /// 会話に使うモデル名。モデルを選べないプロバイダは None
    #[serde(default)]
    pub model: Option<String>,
}

impl Session {
    /// 新しいセッションを作成し、会話に使うプロバイダとモデルを記録する
    #[must_use]
    pub fn new(
        system_prompt: Option<String>,
        ttl_secs: Option<u64>,
        provider: &dyn AiProvider,
    ) -> Self {
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
            provider: Some(provider.name().to_string()),
            model: provider.model().map(String::from),
        }
    }

    /// 会話に使うプロバイダ名。未記録のセッションは Apple Intelligence を返す
    #[must_use]
    pub fn provider_name(&self) -> &str {
        self.provider.as_deref().unwrap_or(APPLE_INTELLIGENCE)
    }

    /// 表示用のプロバイダ名（例: `lm-studio (qwen/qwen3.6-35b-a3b)`）
    #[must_use]
    pub fn provider_label(&self) -> String {
        describe_provider(self.provider_name(), self.model.as_deref())
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
        let filename = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().to_string(),
        );
        self.file_contexts.push(FileContext { filename, content });
        Ok(())
    }

    /// ユーザー入力を送信し、AI の応答を返す
    ///
    /// セッションに記録されたものと異なるプロバイダ・モデルでは送信せず、履歴も変更しない。
    /// 生成に失敗したときも履歴は変更しない。
    pub fn send(
        &mut self,
        provider: &dyn AiProvider,
        input: &str,
    ) -> Result<String, ProviderError> {
        self.ensure_same_provider(provider)?;

        let user_message = Message {
            role: Role::User,
            content: input.to_string(),
            timestamp: Utc::now(),
        };
        let mut messages = self.messages.clone();
        messages.push(user_message.clone());

        let request = GenerateRequest {
            system_prompt: self.system_prompt.clone(),
            messages,
            file_contexts: self.file_contexts.clone(),
            response_schema: None,
        };

        // 失敗した入力を履歴に残すと、送り直したときに user が連続した履歴になり、
        // 役割の交互を求めるチャットテンプレートでは以後の送信がすべて失敗する
        let response = provider.generate(&request)?;

        self.messages.push(user_message);
        self.messages.push(Message {
            role: Role::Assistant,
            content: response.content.clone(),
            timestamp: Utc::now(),
        });

        self.last_active = Utc::now();

        Ok(response.content)
    }

    /// CLI と GUI はセッションの保存先を共有するため、別のプロバイダで作られたセッションに
    /// 別のモデルの応答が混ざらないよう照合する
    fn ensure_same_provider(&self, provider: &dyn AiProvider) -> Result<(), ProviderError> {
        let is_same_provider =
            self.provider_name() == provider.name() && self.model.as_deref() == provider.model();
        if is_same_provider {
            return Ok(());
        }
        Err(ProviderError::Other(format!(
            "session {} was created with {}, but the current provider is {}",
            self.id,
            self.provider_label(),
            describe_provider(provider.name(), provider.model())
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockProvider;

    #[test]
    fn new_records_provider_and_model() {
        let provider = MockProvider::new("lm-studio", Some("test/model"));
        let session = Session::new(None, None, &provider);
        assert_eq!(session.provider.as_deref(), Some("lm-studio"));
        assert_eq!(session.model.as_deref(), Some("test/model"));
        assert_eq!(session.provider_label(), "lm-studio (test/model)");
    }

    #[test]
    fn unrecorded_provider_is_apple_intelligence() {
        let mut session = Session::new(None, None, &MockProvider::default());
        session.provider = None;
        assert_eq!(session.provider_name(), APPLE_INTELLIGENCE);

        let apple = MockProvider::new(APPLE_INTELLIGENCE, None);
        session.send(&apple, "hello").unwrap();
        assert_eq!(session.messages.len(), 2);
    }

    #[test]
    fn send_appends_messages() {
        let provider = MockProvider::default();
        let mut session = Session::new(None, None, &provider);

        let reply = session.send(&provider, "hello").unwrap();
        assert_eq!(reply, "echo: hello");
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[1].role, Role::Assistant);
    }

    #[test]
    fn send_preserves_history() {
        let provider = MockProvider::default();
        let mut session = Session::new(Some("system".to_string()), None, &provider);

        session.send(&provider, "first").unwrap();
        session.send(&provider, "second").unwrap();

        assert_eq!(session.messages.len(), 4);
        assert_eq!(session.messages[2].content, "second");
    }

    #[test]
    fn send_rejects_other_provider_without_touching_history() {
        let lm_studio = MockProvider::new("lm-studio", Some("test/model"));
        let mut session = Session::new(None, None, &lm_studio);
        session.send(&lm_studio, "first").unwrap();
        let messages_before = session.messages.len();
        let last_active_before = session.last_active;

        for other in [
            MockProvider::new(APPLE_INTELLIGENCE, None),
            MockProvider::new("lm-studio", Some("other/model")),
            MockProvider::new("lm-studio", None),
        ] {
            let error = session.send(&other, "second").unwrap_err();
            assert!(error.to_string().contains("lm-studio (test/model)"));
            assert_eq!(session.messages.len(), messages_before);
            assert_eq!(session.last_active, last_active_before);
        }
    }

    #[test]
    fn failed_send_leaves_history_untouched() {
        let provider = MockProvider::default();
        let mut session = Session::new(None, None, &provider);
        session.send(&provider, "first").unwrap();
        let last_active_before = session.last_active;

        let failing = MockProvider::failing();
        session.send(&failing, "second").unwrap_err();
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.last_active, last_active_before);

        session.send(&provider, "retry").unwrap();
        let roles: Vec<Role> = session.messages.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            [Role::User, Role::Assistant, Role::User, Role::Assistant]
        );
    }

    #[test]
    fn is_expired_without_ttl() {
        let session = Session::new(None, None, &MockProvider::default());
        assert!(!session.is_expired());
    }

    #[test]
    fn is_expired_with_future_ttl() {
        let session = Session::new(None, Some(3600), &MockProvider::default());
        assert!(!session.is_expired());
    }

    #[test]
    fn is_expired_with_past_ttl() {
        let mut session = Session::new(None, Some(0), &MockProvider::default());
        // Force last_active to the past
        session.last_active = Utc::now() - chrono::Duration::seconds(10);
        assert!(session.is_expired());
    }
}
