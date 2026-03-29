//! Swift FFI 関数宣言と JSON レスポンスパース
//!
//! Swift 側は JSON 文字列で結果を返す:
//! - 成功: `{"ok":"text"}`
//! - 失敗: `{"error":"type","message":"msg"}`

use serde::{Deserialize, Serialize};
use swift_rs::{swift, Bool, SRString};

swift!(fn ai_check_availability() -> Bool);
swift!(fn ai_generate(system: &SRString, user: &SRString) -> SRString);
swift!(fn ai_generate_with_history(payload: &SRString) -> SRString);

#[derive(Deserialize)]
struct AiOk {
    ok: String,
}

#[derive(Deserialize)]
struct AiErr {
    error: String,
    message: String,
}

#[derive(Serialize)]
struct ChatPayload<'a> {
    system: Option<&'a str>,
    messages: &'a [ChatMessage],
}

#[derive(Serialize)]
pub struct ChatMessage {
    pub role: &'static str,
    pub content: String,
}

/// Apple Intelligence の利用可否を返す（非ブロッキング）
pub fn is_available() -> bool {
    // Safety: FFI call with no mutable state; safe to call from any thread.
    unsafe { ai_check_availability() }
}

/// Apple Intelligence でテキスト生成を行う（ブロッキング、シングルターン）
///
/// メインスレッドから呼び出すとデッドロックする可能性がある。
pub fn generate(system: &str, user: &str) -> Result<String, String> {
    let system = SRString::from(system);
    let user = SRString::from(user);

    // Safety: FFI call that blocks via DispatchSemaphore internally.
    // Must not be called from the main thread.
    let json = unsafe { ai_generate(&system, &user) };
    parse_response(json.as_str())
}

/// Apple Intelligence でマルチターン生成を行う（ブロッキング）
///
/// 会話履歴を JSON payload として渡し、Swift 側で LanguageModelSession に
/// 逐次 respond して文脈を構築する。
pub fn generate_with_history(
    system: Option<&str>,
    messages: &[ChatMessage],
) -> Result<String, String> {
    let payload = ChatPayload { system, messages };
    let json_str =
        serde_json::to_string(&payload).map_err(|e| format!("failed to serialize payload: {e}"))?;
    let sr = SRString::from(json_str.as_str());

    // Safety: FFI call that blocks via DispatchSemaphore internally.
    let json = unsafe { ai_generate_with_history(&sr) };
    parse_response(json.as_str())
}

fn parse_response(json_str: &str) -> Result<String, String> {
    if let Ok(ok) = serde_json::from_str::<AiOk>(json_str) {
        return Ok(ok.ok);
    }
    if let Ok(err) = serde_json::from_str::<AiErr>(json_str) {
        return Err(format!("{}: {}", err.error, err.message));
    }
    Err(format!("unexpected response from AI bridge: {json_str}"))
}
