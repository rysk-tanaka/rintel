use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// AI プロバイダへのリクエスト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub system_prompt: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub file_contexts: Vec<FileContext>,
}

/// ファイルコンテキスト（プロンプトに注入されるファイル内容）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContext {
    pub filename: String,
    pub content: String,
}

/// 会話メッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    /// メッセージ作成時刻（既存データとの互換のため省略可）
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
}

/// メッセージの送信者
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// AI プロバイダからのレスポンス
#[derive(Debug, Clone)]
pub struct GenerateResponse {
    pub content: String,
    pub provider: String,
}

/// プロバイダエラー
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("provider not available")]
    NotAvailable,

    #[error("generation failed: {0}")]
    GenerationFailed(String),

    #[error("{0}")]
    Other(String),
}
