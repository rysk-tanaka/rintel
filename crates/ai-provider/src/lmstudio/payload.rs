//! chat completions のリクエスト本文の組み立てと応答の解釈。ネットワークには触れない

use serde_json::{Value, json};

use super::config::LmStudioConfig;
use super::truncate_for_error;
use crate::prompt::build_file_prefix;
use crate::types::{GenerateRequest, ProviderError, Role};

/// `POST /chat/completions` の本文を組み立てる
pub(crate) fn build_request_body(
    config: &LmStudioConfig,
    request: &GenerateRequest,
) -> Result<Value, ProviderError> {
    // Apple Intelligence と挙動を揃え、構造化生成はシングルターンに限る
    if request.response_schema.is_some() && request.messages.len() > 1 {
        return Err(generation_failed(
            "structured generation only supports single-turn requests",
        ));
    }
    let schema = request
        .response_schema
        .as_deref()
        .map(parse_schema)
        .transpose()?;

    let mut body = json!({
        "model": config.model,
        "messages": build_messages(request)?,
        "stream": false,
        "max_tokens": config.max_tokens,
        "ttl": config.ttl_secs,
    });
    // LM Studio が思考を止めるのは "none" だけで、low/medium/high はどれも同じように思考する
    if !config.thinking {
        body["reasoning_effort"] = json!("none");
    }
    if let Some(schema) = schema {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": {"name": "response", "strict": true, "schema": schema},
        });
    }
    Ok(body)
}

/// 応答から本文を取り出す
pub(crate) fn parse_completion(
    response: &Value,
    config: &LmStudioConfig,
    is_structured: bool,
) -> Result<String, ProviderError> {
    let choice = &response["choices"][0];
    if !choice["message"].is_object() {
        return Err(generation_failed(format!(
            "unexpected response from LM Studio: {}",
            truncate_for_error(&response.to_string())
        )));
    }
    ensure_answered_by(response, &config.model)?;
    // 上限で切れた出力を成功扱いにしない。結論に届かなかった思考もこれに当たる
    if choice["finish_reason"] == "length" {
        let hint = if config.thinking {
            ", or retry without RINTEL_LMS_THINKING"
        } else {
            ""
        };
        return Err(generation_failed(format!(
            "model output was truncated at the token limit (finish_reason: length, max {}); raise RINTEL_LMS_MAX_TOKENS{hint}",
            config.max_tokens
        )));
    }

    let content = strip_leading_think(choice["message"]["content"].as_str().unwrap_or_default());
    if content.trim().is_empty() {
        return Err(generation_failed(format!(
            "empty response from model '{}'",
            config.model
        )));
    }
    // 呼び出し側（dotfiles のタスク等）は失敗時にリトライするため、JSON でなければ失敗として返す
    if is_structured && serde_json::from_str::<Value>(content).is_err() {
        return Err(generation_failed(
            "model output is not valid JSON despite the response schema",
        ));
    }
    Ok(content.to_string())
}

/// 応答したモデルが指定したモデルかを確かめる
///
/// 埋め込みモデルのように一覧にあってもチャットできないキーを指定すると、LM Studio は
/// ロード済みの別モデルで黙って応答する。応答の `model` には実際に応答したモデルが入る。
fn ensure_answered_by(response: &Value, requested: &str) -> Result<(), ProviderError> {
    let answered = response["model"].as_str();
    if answered.is_some_and(|answered| is_same_model(answered, requested)) {
        return Ok(());
    }
    Err(generation_failed(format!(
        "LM Studio answered with model '{}' instead of '{requested}'; '{requested}' may not be a chat model (e.g. an embedding model)",
        answered.unwrap_or("unknown")
    )))
}

/// 同じモデルを複数ロードすると、2 つ目以降は `{key}:2` のようなインスタンス名で応答することがある
fn is_same_model(answered: &str, requested: &str) -> bool {
    let Some(suffix) = answered.strip_prefix(requested) else {
        return false;
    };
    let is_exact = suffix.is_empty();
    let is_numbered_instance = suffix
        .strip_prefix(':')
        .is_some_and(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()));
    is_exact || is_numbered_instance
}

fn build_messages(request: &GenerateRequest) -> Result<Vec<Value>, ProviderError> {
    let first_user = request
        .messages
        .iter()
        .position(|message| message.role == Role::User)
        .ok_or_else(|| generation_failed("request has no user message"))?;
    let file_prefix = build_file_prefix(&request.file_contexts);

    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    if let Some(system) = request.system_prompt.as_deref().filter(|s| !s.is_empty()) {
        messages.push(json!({"role": "system", "content": system}));
    }
    for (i, message) in request.messages.iter().enumerate() {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        // Apple Intelligence のマルチターンと同じく、ファイルは最初の user メッセージにだけ付ける
        let content = if i == first_user {
            format!("{file_prefix}{}", message.content)
        } else {
            message.content.clone()
        };
        messages.push(json!({"role": role, "content": content}));
    }
    Ok(messages)
}

fn parse_schema(schema: &str) -> Result<Value, ProviderError> {
    serde_json::from_str(schema)
        .map_err(|error| generation_failed(format!("response schema is not valid JSON: {error}")))
}

/// 先頭の `<think>…</think>` だけを除く
///
/// 本文の途中にある `</think>` は引用の可能性があり、そこまで除くと本文を失う。
/// 閉じタグがない場合も手を付けない。
fn strip_leading_think(content: &str) -> &str {
    let Some(rest) = content.trim_start().strip_prefix("<think>") else {
        return content;
    };
    match rest.find("</think>") {
        Some(end) => rest[end + "</think>".len()..].trim_start(),
        None => content,
    }
}

fn generation_failed(message: impl Into<String>) -> ProviderError {
    ProviderError::GenerationFailed(message.into())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::types::{FileContext, Message};

    fn config(thinking: bool) -> LmStudioConfig {
        let thinking = thinking.to_string();
        LmStudioConfig::from_lookup("test/model".to_string(), |key| {
            (key == "RINTEL_LMS_THINKING").then(|| thinking.clone())
        })
        .unwrap()
    }

    fn message(role: Role, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
        }
    }

    fn request(messages: Vec<Message>) -> GenerateRequest {
        GenerateRequest {
            system_prompt: None,
            messages,
            file_contexts: Vec::new(),
            response_schema: None,
        }
    }

    fn completion(content: &str, finish_reason: &str) -> Value {
        json!({
            "model": "test/model",
            "choices": [{
                "message": {"role": "assistant", "content": content},
                "finish_reason": finish_reason,
            }]
        })
    }

    #[test]
    fn body_disables_thinking_by_default() {
        let body =
            build_request_body(&config(false), &request(vec![message(Role::User, "hi")])).unwrap();
        assert_eq!(body["model"], "test/model");
        assert_eq!(body["stream"], false);
        assert_eq!(body["max_tokens"], 16_384);
        assert_eq!(body["ttl"], 600);
        assert_eq!(body["reasoning_effort"], "none");
        assert!(body.get("response_format").is_none());
        assert_eq!(body["messages"], json!([{"role": "user", "content": "hi"}]));
    }

    #[test]
    fn body_omits_reasoning_effort_when_thinking() {
        let body =
            build_request_body(&config(true), &request(vec![message(Role::User, "hi")])).unwrap();
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn body_puts_system_first_and_files_on_first_user_message() {
        let mut request = request(vec![
            message(Role::User, "first"),
            message(Role::Assistant, "reply"),
            message(Role::User, "second"),
        ]);
        request.system_prompt = Some("be brief".to_string());
        request.file_contexts = vec![FileContext {
            filename: "a.rs".to_string(),
            content: "fn main() {}".to_string(),
        }];

        let body = build_request_body(&config(false), &request).unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 4);
        assert_eq!(
            messages[0],
            json!({"role": "system", "content": "be brief"})
        );
        let first = messages[1]["content"].as_str().unwrap();
        assert!(first.starts_with("--- Reference Files ---"));
        assert!(first.contains("fn main() {}"));
        assert!(first.ends_with("first"));
        assert_eq!(
            messages[2],
            json!({"role": "assistant", "content": "reply"})
        );
        assert_eq!(messages[3], json!({"role": "user", "content": "second"}));
    }

    #[test]
    fn body_skips_empty_system_prompt() {
        let mut request = request(vec![message(Role::User, "hi")]);
        request.system_prompt = Some(String::new());
        let body = build_request_body(&config(false), &request).unwrap();
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn body_includes_json_schema() {
        let mut request = request(vec![message(Role::User, "hi")]);
        request.response_schema = Some(r#"{"type": "object"}"#.to_string());
        let body = build_request_body(&config(false), &request).unwrap();
        assert_eq!(
            body["response_format"],
            json!({
                "type": "json_schema",
                "json_schema": {"name": "response", "strict": true, "schema": {"type": "object"}},
            })
        );
    }

    #[test]
    fn body_rejects_invalid_schema() {
        let mut request = request(vec![message(Role::User, "hi")]);
        request.response_schema = Some("{not json".to_string());
        let error = build_request_body(&config(false), &request).unwrap_err();
        assert!(error.to_string().contains("schema"));
    }

    #[test]
    fn body_rejects_schema_with_history() {
        let mut request = request(vec![
            message(Role::User, "first"),
            message(Role::Assistant, "reply"),
            message(Role::User, "second"),
        ]);
        request.response_schema = Some(r#"{"type": "object"}"#.to_string());
        let error = build_request_body(&config(false), &request).unwrap_err();
        assert!(error.to_string().contains("single-turn"));
    }

    #[test]
    fn body_rejects_request_without_user_message() {
        let error = build_request_body(&config(false), &request(Vec::new())).unwrap_err();
        assert!(error.to_string().contains("no user message"));
    }

    #[test]
    fn parses_content() {
        let content =
            parse_completion(&completion("hello", "stop"), &config(false), false).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn rejects_truncated_output() {
        let error =
            parse_completion(&completion("partial", "length"), &config(false), false).unwrap_err();
        assert!(error.to_string().contains("RINTEL_LMS_MAX_TOKENS"));
    }

    #[test]
    fn rejects_empty_content() {
        for response in [
            completion("", "stop"),
            completion("  \n", "stop"),
            completion("<think>only thoughts</think>", "stop"),
            json!({
                "model": "test/model",
                "choices": [{"message": {"content": null}, "finish_reason": "stop"}],
            }),
        ] {
            let error = parse_completion(&response, &config(false), false).unwrap_err();
            assert!(error.to_string().contains("empty response"), "{response}");
        }
    }

    #[test]
    fn rejects_response_without_message() {
        for response in [json!({}), json!({"choices": []}), json!("<html>")] {
            let error = parse_completion(&response, &config(false), false).unwrap_err();
            assert!(
                error.to_string().contains("unexpected response"),
                "{response}"
            );
        }
    }

    #[test]
    fn rejects_answer_from_other_model() {
        let mut response = completion("hello", "stop");
        response["model"] = json!("other/model");
        let error = parse_completion(&response, &config(false), false)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("'other/model' instead of 'test/model'"),
            "{error}"
        );

        response.as_object_mut().unwrap().remove("model");
        let error = parse_completion(&response, &config(false), false)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("'unknown' instead of 'test/model'"),
            "{error}"
        );
    }

    #[test]
    fn matches_model_instances() {
        assert!(is_same_model("test/model", "test/model"));
        assert!(is_same_model("test/model:2", "test/model"));
        assert!(!is_same_model("test/model:", "test/model"));
        assert!(!is_same_model("test/model:q4", "test/model"));
        assert!(!is_same_model("test/model-v2", "test/model"));
        assert!(!is_same_model("test", "test/model"));
    }

    #[test]
    fn strips_only_leading_think_block() {
        assert_eq!(
            strip_leading_think("<think>plan</think>\n\nanswer"),
            "answer"
        );
        assert_eq!(
            strip_leading_think("  <think>a</think>b</think>c"),
            "b</think>c"
        );
        assert_eq!(
            strip_leading_think("quote </think> stays"),
            "quote </think> stays"
        );
        assert_eq!(strip_leading_think("<think>unclosed"), "<think>unclosed");
    }

    #[test]
    fn structured_output_must_be_json() {
        let error =
            parse_completion(&completion("not json", "stop"), &config(false), true).unwrap_err();
        assert!(error.to_string().contains("not valid JSON"));

        let content =
            parse_completion(&completion(r#"{"a": 1}"#, "stop"), &config(false), true).unwrap();
        assert_eq!(content, r#"{"a": 1}"#);
    }
}
