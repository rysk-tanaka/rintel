//! LM Studio への HTTP 呼び出し（ureq の同期クライアント）

use std::time::Duration;

use serde::Deserialize;
use ureq::http::Response;
use ureq::{Agent, Body, RequestBuilder};

use super::truncate_for_error;
use crate::types::ProviderError;

/// リクエストに付ける認証
#[derive(Clone)]
pub(crate) enum Auth {
    /// トークン関連の環境変数が両方未設定のときは `Authorization` ヘッダーを付けない
    None,
    Bearer(String),
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

pub(crate) fn agent() -> Agent {
    Agent::config_builder()
        // ステータスに関わらず本文を読み、LM Studio のエラー内容（モデル未ロード等）を伝える
        .http_status_as_error(false)
        // Bearer トークンをリダイレクト先へ送らない。LM Studio はリダイレクトしない
        .max_redirects(0)
        // ureq は既定で HTTP(S)_PROXY / ALL_PROXY をスキームを問わず使い、平文のトークンがプロキシに渡る。
        // LM Studio はローカルか tailnet 上にあり、プロキシを経由する必要はない
        .proxy(None)
        .build()
        .into()
}

/// サーバーが HTTP で応答するかを確かめる。ステータスは問わず、トークンも送らない
pub(crate) fn probe(agent: &Agent, base_url: &str, timeout: Duration) -> bool {
    agent
        .get(format!("{base_url}/models"))
        .config()
        .timeout_global(Some(timeout))
        .build()
        .call()
        .is_ok()
}

/// 利用可能なモデルキー（`data[].id`）の一覧を取得する
pub(crate) fn list_models(
    agent: &Agent,
    base_url: &str,
    auth: &Auth,
    timeout: Duration,
) -> Result<Vec<String>, ProviderError> {
    let url = format!("{base_url}/models");
    let result = with_auth(agent.get(&url), auth)
        .config()
        .timeout_global(Some(timeout))
        .build()
        .call();
    let body = read_success_body(&url, result)?;
    let list: ModelList =
        serde_json::from_str(&body).map_err(|_| unexpected_response(&url, &body))?;
    Ok(list.data.into_iter().map(|model| model.id).collect())
}

/// `POST /chat/completions` を送り、応答 JSON を返す
pub(crate) fn chat_completion(
    agent: &Agent,
    base_url: &str,
    auth: &Auth,
    body: &str,
    timeout: Duration,
) -> Result<serde_json::Value, ProviderError> {
    let url = format!("{base_url}/chat/completions");
    let result = with_auth(agent.post(&url), auth)
        .header("Content-Type", "application/json")
        .config()
        .timeout_global(Some(timeout))
        .build()
        .send(body);
    let text = read_success_body(&url, result)?;
    // LM Studio の前段のプロキシが 200 で HTML を返すこともあるため、本文を添えて報告する
    serde_json::from_str(&text).map_err(|_| unexpected_response(&url, &text))
}

fn with_auth<B>(request: RequestBuilder<B>, auth: &Auth) -> RequestBuilder<B> {
    match auth {
        Auth::None => request,
        Auth::Bearer(token) => request.header("Authorization", format!("Bearer {token}")),
    }
}

/// 2xx なら本文を返し、それ以外は本文の先頭を含むエラーにする
fn read_success_body(
    url: &str,
    result: Result<Response<Body>, ureq::Error>,
) -> Result<String, ProviderError> {
    let mut response = result.map_err(|error| request_failed(url, &error))?;
    let status = response.status();
    let body = response.body_mut().read_to_string();
    if !status.is_success() {
        // 本文を読み切れなくてもステータスは伝える。401 なら認証の問題だと分かる
        let detail = match &body {
            Ok(text) => truncate_for_error(text),
            Err(error) => format!("(failed to read response body: {error})"),
        };
        return Err(ProviderError::GenerationFailed(format!(
            "request to {url} failed with HTTP {}: {detail}",
            status.as_u16()
        )));
    }
    body.map_err(|error| request_failed(url, &error))
}

fn request_failed(url: &str, error: &ureq::Error) -> ProviderError {
    ProviderError::GenerationFailed(format!("request to {url} failed: {error}"))
}

fn unexpected_response(url: &str, body: &str) -> ProviderError {
    ProviderError::GenerationFailed(format!(
        "unexpected response from {url}: {}",
        truncate_for_error(body)
    ))
}
