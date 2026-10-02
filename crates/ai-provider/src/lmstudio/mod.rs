//! LM Studio（OpenAI 互換 API）プロバイダ
//!
//! 接続設定は rysk-tanaka/skills の LM Studio クライアントと共通の
//! `LM_API_URL` / `LM_API_TOKEN` / `LM_API_TOKEN_COMMAND` に従う。

mod client;
mod config;
mod payload;
mod provider;
#[cfg(test)]
mod tests;

pub use config::{DEFAULT_MODEL, LmStudioConfig};
pub use provider::LmStudioProvider;

/// エラー文に含めるレスポンス本文の上限（文字数）
const ERROR_BODY_LIMIT: usize = 1000;

/// エラー文に載せるため、長い本文を切り詰める
fn truncate_for_error(text: &str) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(ERROR_BODY_LIMIT).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}
