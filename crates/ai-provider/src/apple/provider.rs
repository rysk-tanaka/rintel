use crate::provider::AiProvider;
use crate::types::{FileContext, GenerateRequest, GenerateResponse, Message, ProviderError, Role};

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
        "apple-intelligence"
    }

    fn is_available(&self) -> bool {
        ffi::is_available()
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        if !self.is_available() {
            return Err(ProviderError::NotAvailable);
        }

        let has_history = request.messages.len() > 1;

        let result = if has_history {
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

/// ファイルコンテキストをプレフィックス文字列に変換する
fn build_file_prefix(files: &[FileContext]) -> String {
    if files.is_empty() {
        return String::new();
    }

    let mut prefix = String::from("--- Reference Files ---\n\n");
    for file in files {
        use std::fmt::Write;
        let _ = writeln!(
            prefix,
            "### {}\n```\n{}\n```\n",
            file.filename, file.content
        );
    }
    prefix.push_str("--- End of Files ---\n\n");
    prefix
}

/// シングルターン用のプロンプト構築
fn build_single_prompt(messages: &[Message], files: &[FileContext]) -> String {
    let file_prefix = build_file_prefix(files);

    if messages.len() == 1 && messages[0].role == Role::User {
        return format!("{file_prefix}{}", messages[0].content);
    }

    let mut prompt = file_prefix;
    for msg in messages {
        let prefix = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        prompt.push_str(&format!("{prefix}: {}\n\n", msg.content));
    }
    prompt
}
