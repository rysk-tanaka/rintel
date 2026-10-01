use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use ureq::http::Uri;

use crate::types::ProviderError;

/// `LM_API_URL` が未設定のときの接続先。rysk-tanaka/skills の LM Studio クライアントと同じ
const DEFAULT_API_URL: &str = "http://localhost:1234";

/// 既定のモデルキー。MoE（active 3B）で軽く速い。lms-review の既定と揃える
pub const DEFAULT_MODEL: &str = "qwen/qwen3.6-35b-a3b";

const DEFAULT_MAX_TOKENS: u32 = 16_384;
const DEFAULT_TIMEOUT_SECS: u64 = 300;
const DEFAULT_TTL_SECS: u64 = 600;

/// 生成タイムアウトの上限。ureq は期限を `Instant + Duration` で計算し、巨大な値ではオーバーフローして panic する
const MAX_TIMEOUT_SECS: u64 = 86_400;

/// スリープ中の Mac mini や Tailscale の切断を素早く見切るための疎通確認の上限
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// 認証付きモデル一覧取得の上限。本文の受信までを含む
const MODELS_TIMEOUT: Duration = Duration::from_secs(10);

/// LM Studio の接続・生成設定
#[derive(Clone)]
pub struct LmStudioConfig {
    /// OpenAI 互換 API のベース URL（`{LM_API_URL}/v1`）
    pub(crate) base_url: String,
    pub(crate) token: Option<String>,
    pub(crate) token_command: Option<String>,
    pub(crate) model: String,
    pub(crate) thinking: bool,
    pub(crate) max_tokens: u32,
    pub(crate) ttl_secs: u64,
    pub(crate) request_timeout: Duration,
    pub(crate) probe_timeout: Duration,
    pub(crate) models_timeout: Duration,
}

impl LmStudioConfig {
    /// `lookup`（通常は環境変数）から設定を読む。`model` は呼び出し側で解決済みのモデルキー
    ///
    /// シェルの `${VAR:-default}` と同じく空文字は未設定として扱う。edition 2024 では
    /// `std::env::set_var` が unsafe なため、テストは環境変数を書き換えずに `lookup` で値を与える。
    pub fn from_lookup(
        model: String,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ProviderError> {
        let get = |key: &str| lookup(key).filter(|value| !value.is_empty());

        if model.is_empty() {
            return Err(config_error("LM Studio model key must not be empty"));
        }
        let api_url = get("LM_API_URL").unwrap_or_else(|| DEFAULT_API_URL.to_string());
        let timeout_secs = parse_positive(
            "RINTEL_LMS_TIMEOUT",
            get("RINTEL_LMS_TIMEOUT"),
            DEFAULT_TIMEOUT_SECS,
        )?;
        if timeout_secs > MAX_TIMEOUT_SECS {
            return Err(config_error(format!(
                "RINTEL_LMS_TIMEOUT must be at most {MAX_TIMEOUT_SECS} seconds (got {timeout_secs})"
            )));
        }

        Ok(Self {
            base_url: to_base_url(&api_url)?,
            token: get("LM_API_TOKEN"),
            token_command: get("LM_API_TOKEN_COMMAND"),
            model,
            thinking: parse_bool("RINTEL_LMS_THINKING", get("RINTEL_LMS_THINKING"))?,
            max_tokens: parse_positive(
                "RINTEL_LMS_MAX_TOKENS",
                get("RINTEL_LMS_MAX_TOKENS"),
                DEFAULT_MAX_TOKENS,
            )?,
            ttl_secs: parse_positive("RINTEL_LMS_TTL", get("RINTEL_LMS_TTL"), DEFAULT_TTL_SECS)?,
            request_timeout: Duration::from_secs(timeout_secs),
            probe_timeout: PROBE_TIMEOUT,
            models_timeout: MODELS_TIMEOUT,
        })
    }
}

// Debug 出力からトークンが漏れないよう、値の有無だけを示す
impl fmt::Debug for LmStudioConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LmStudioConfig")
            .field("base_url", &self.base_url)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("token_command", &self.token_command)
            .field("model", &self.model)
            .field("thinking", &self.thinking)
            .field("max_tokens", &self.max_tokens)
            .field("ttl_secs", &self.ttl_secs)
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}

/// `LM_API_URL`（サーバーのルート URL）から OpenAI 互換 API のベース URL を作る
fn to_base_url(api_url: &str) -> Result<String, ProviderError> {
    // URL に埋め込んだ認証情報はエラー文に出てしまう。LM Studio の認証は Bearer トークンで足りるため受け付けず、
    // このエラー文にも URL を含めない
    if api_url.contains('@') {
        return Err(config_error(
            "LM_API_URL must not contain credentials; use LM_API_TOKEN or LM_API_TOKEN_COMMAND",
        ));
    }

    // LM Studio は HTTP で待ち受けるため TLS を組み込んでいない。https は接続失敗ではなく設定エラーにする。
    // パスは後ろに `/v1` 等を連結するため、クエリやフラグメントがあると URL が壊れる
    let is_valid = api_url.parse::<Uri>().is_ok_and(|uri| {
        let is_http = uri.scheme_str() == Some("http");
        let has_host = uri.host().is_some_and(|host| !host.is_empty());
        let has_query_or_fragment = uri.query().is_some() || api_url.contains('#');
        is_http && has_host && !has_query_or_fragment
    });
    if !is_valid {
        return Err(config_error(format!(
            "LM_API_URL must be an http:// URL with a host and no query (got {api_url})"
        )));
    }

    let root = api_url.trim_end_matches('/');
    // `LM_API_URL` には `/v1` を含まないルート URL を指定する決まりだが、付けて設定されても二重にしない
    if root.ends_with("/v1") {
        Ok(root.to_string())
    } else {
        Ok(format!("{root}/v1"))
    }
}

fn parse_bool(name: &str, value: Option<String>) -> Result<bool, ProviderError> {
    match value.as_deref() {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(other) => Err(config_error(format!(
            "{name} must be true or false (got {other})"
        ))),
    }
}

fn parse_positive<T>(name: &str, value: Option<String>, default: T) -> Result<T, ProviderError>
where
    T: FromStr + PartialEq + From<u8>,
{
    let Some(value) = value else {
        return Ok(default);
    };
    match value.parse::<T>() {
        Ok(parsed) if parsed != T::from(0) => Ok(parsed),
        _ => Err(config_error(format!(
            "{name} must be a positive integer (got {value})"
        ))),
    }
}

fn config_error(message: impl Into<String>) -> ProviderError {
    ProviderError::Other(format!(
        "invalid LM Studio configuration: {}",
        message.into()
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(env: &[(&str, &str)]) -> Result<LmStudioConfig, ProviderError> {
        let env: HashMap<String, String> = env
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        LmStudioConfig::from_lookup("test/model".to_string(), |key| env.get(key).cloned())
    }

    fn error_message(env: &[(&str, &str)]) -> String {
        config(env).unwrap_err().to_string()
    }

    #[test]
    fn defaults_follow_lms_review() {
        let config = config(&[]).unwrap();
        assert_eq!(config.base_url, "http://localhost:1234/v1");
        assert_eq!(config.model, "test/model");
        assert_eq!(config.token, None);
        assert_eq!(config.token_command, None);
        assert!(!config.thinking);
        assert_eq!(config.max_tokens, 16_384);
        assert_eq!(config.ttl_secs, 600);
        assert_eq!(config.request_timeout, Duration::from_secs(300));
        assert_eq!(config.probe_timeout, Duration::from_secs(5));
        assert_eq!(config.models_timeout, Duration::from_secs(10));
    }

    #[test]
    fn reads_all_settings() {
        let config = config(&[
            ("LM_API_URL", "http://m6m:1234"),
            ("LM_API_TOKEN", "sk-lm-test"),
            ("LM_API_TOKEN_COMMAND", "printf token"),
            ("RINTEL_LMS_THINKING", "true"),
            ("RINTEL_LMS_MAX_TOKENS", "2048"),
            ("RINTEL_LMS_TIMEOUT", "30"),
            ("RINTEL_LMS_TTL", "60"),
        ])
        .unwrap();
        assert_eq!(config.base_url, "http://m6m:1234/v1");
        assert_eq!(config.token.as_deref(), Some("sk-lm-test"));
        assert_eq!(config.token_command.as_deref(), Some("printf token"));
        assert!(config.thinking);
        assert_eq!(config.max_tokens, 2048);
        assert_eq!(config.request_timeout, Duration::from_secs(30));
        assert_eq!(config.ttl_secs, 60);
    }

    #[test]
    fn empty_values_are_unset() {
        let config = config(&[
            ("LM_API_URL", ""),
            ("LM_API_TOKEN", ""),
            ("LM_API_TOKEN_COMMAND", ""),
            ("RINTEL_LMS_MAX_TOKENS", ""),
        ])
        .unwrap();
        assert_eq!(config.base_url, "http://localhost:1234/v1");
        assert_eq!(config.token, None);
        assert_eq!(config.token_command, None);
        assert_eq!(config.max_tokens, 16_384);
    }

    #[test]
    fn normalizes_base_url() {
        for (api_url, expected) in [
            ("http://m6m:1234/", "http://m6m:1234/v1"),
            ("http://m6m:1234//", "http://m6m:1234/v1"),
            ("http://m6m:1234/v1", "http://m6m:1234/v1"),
            ("http://m6m:1234/v1/", "http://m6m:1234/v1"),
            ("http://127.0.0.1:1234", "http://127.0.0.1:1234/v1"),
            ("http://m6m:1234/lmstudio", "http://m6m:1234/lmstudio/v1"),
        ] {
            let config = config(&[("LM_API_URL", api_url)]).unwrap();
            assert_eq!(config.base_url, expected, "LM_API_URL={api_url}");
        }
    }

    #[test]
    fn rejects_non_http_urls() {
        for api_url in [
            "https://m6m:1234",
            "m6m:1234",
            "http://",
            "http:///",
            "http:///foo",
            "http://:1234",
            "http://m6m:1234?x=1",
            "http://m6m:1234/#top",
        ] {
            let message = error_message(&[("LM_API_URL", api_url)]);
            assert!(message.contains("LM_API_URL"), "{api_url}: {message}");
        }
    }

    #[test]
    fn rejects_credentials_without_echoing_them() {
        for api_url in ["http://user:secret@m6m:1234", "user:secret@m6m:1234"] {
            let message = error_message(&[("LM_API_URL", api_url)]);
            assert!(message.contains("credentials"), "{message}");
            assert!(!message.contains("secret"), "{message}");
        }
    }

    #[test]
    fn rejects_invalid_values() {
        for (name, value) in [
            ("RINTEL_LMS_THINKING", "yes"),
            ("RINTEL_LMS_MAX_TOKENS", "0"),
            ("RINTEL_LMS_MAX_TOKENS", "abc"),
            ("RINTEL_LMS_MAX_TOKENS", "-1"),
            ("RINTEL_LMS_TIMEOUT", "0"),
            ("RINTEL_LMS_TIMEOUT", "1.5"),
            ("RINTEL_LMS_TIMEOUT", "86401"),
            ("RINTEL_LMS_TIMEOUT", "18446744073709551615"),
            ("RINTEL_LMS_TTL", "0"),
        ] {
            let message = error_message(&[(name, value)]);
            assert!(message.contains(name), "{name}={value}: {message}");
        }
    }

    #[test]
    fn accepts_timeout_up_to_one_day() {
        let config = config(&[("RINTEL_LMS_TIMEOUT", "86400")]).unwrap();
        assert_eq!(config.request_timeout, Duration::from_secs(86_400));
    }

    #[test]
    fn rejects_empty_model() {
        let error = LmStudioConfig::from_lookup(String::new(), |_| None).unwrap_err();
        assert!(error.to_string().contains("model"));
    }

    #[test]
    fn debug_redacts_token() {
        let config = config(&[("LM_API_TOKEN", "sk-lm-secret")]).unwrap();
        let debug = format!("{config:?}");
        assert!(!debug.contains("sk-lm-secret"));
        assert!(debug.contains("<redacted>"));
    }
}
