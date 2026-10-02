use crate::types::{GenerateRequest, GenerateResponse, ProviderError};

/// AI プロバイダの共通インターフェース
///
/// 各プロバイダはステートレスに動作する。会話履歴は `GenerateRequest::messages` で毎回渡す。
/// `generate()` は同期関数であり、呼び出し元がスレッド管理を行うこと。
pub trait AiProvider: Send + Sync {
    /// プロバイダ名（例: "apple-intelligence"）
    fn name(&self) -> &str;

    /// 生成に使うモデル名。モデルを選べないプロバイダは None
    fn model(&self) -> Option<&str> {
        None
    }

    /// このプロバイダが現在の環境で利用可能か
    fn is_available(&self) -> bool;

    /// `is_available()` が false のときに利用者へ示す説明
    fn unavailable_message(&self) -> String {
        format!("{} is not available", self.name())
    }

    /// テキスト生成を実行する（ブロッキング呼び出し）
    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ProviderError>;
}
