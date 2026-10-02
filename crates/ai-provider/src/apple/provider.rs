use crate::prompt::{build_file_prefix, build_single_prompt};
use crate::provider::AiProvider;
use crate::types::{GenerateRequest, GenerateResponse, ProviderError, Role};

use super::ffi::{self, ChatMessage};

/// Apple Intelligence (Foundation Models) プロバイダ
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
        ffi::is_available()
    }

    fn unavailable_message(&self) -> String {
        "Apple Intelligence is not available on this system.".to_string()
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        if !self.is_available() {
            return Err(ProviderError::NotAvailable);
        }

        let has_history = request.messages.len() > 1;

        // 構造化生成はシングルターン専用。履歴付きで schema を渡されたら平坦化して黙って
        // 履歴を捨てるのではなく、明示的に拒否する（マルチターンは ai_generate_with_history
        // 経由で履歴を再現すべき、というリポジトリ規約に沿う）。
        let result = if let Some(schema) = request.response_schema.as_deref() {
            if has_history {
                Err("structured generation only supports single-turn requests".to_string())
            } else {
                generate_structured(request, schema)
            }
        } else if has_history {
            generate_multi_turn(request)
        } else {
            generate_single_turn(request)
        };

        result
            .map(|content| GenerateResponse {
                content,
                provider: self.name().to_string(),
            })
            .map_err(ProviderError::GenerationFailed)
    }
}

/// シングルターン生成（メッセージ 1 件、または後方互換）
fn generate_single_turn(request: &GenerateRequest) -> Result<String, String> {
    let system = request.system_prompt.as_deref().unwrap_or("");
    let user_prompt = build_single_prompt(&request.messages, &request.file_contexts);
    ffi::generate(system, &user_prompt)
}

/// 構造化生成（JSON Schema 準拠の JSON を返す、シングルターン）
fn generate_structured(request: &GenerateRequest, schema: &str) -> Result<String, String> {
    let system = request.system_prompt.as_deref().unwrap_or("");
    let user_prompt = build_single_prompt(&request.messages, &request.file_contexts);
    ffi::generate_structured(system, &user_prompt, schema)
}

/// マルチターン生成（LanguageModelSession で会話履歴を再現）
fn generate_multi_turn(request: &GenerateRequest) -> Result<String, String> {
    let mut chat_messages = Vec::new();

    // ファイルコンテキストがある場合、最初の user メッセージに注入
    let file_prefix = build_file_prefix(&request.file_contexts);

    for (i, msg) in request.messages.iter().enumerate() {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        let content = if i == 0 && !file_prefix.is_empty() && msg.role == Role::User {
            format!("{file_prefix}{}", msg.content)
        } else {
            msg.content.clone()
        };
        chat_messages.push(ChatMessage { role, content });
    }

    ffi::generate_with_history(request.system_prompt.as_deref(), &chat_messages)
}
