use std::process::{Command, Stdio};
use std::sync::{Mutex, PoisonError};

use ureq::Agent;

use super::client::{self, Auth};
use super::config::LmStudioConfig;
use super::payload;
use crate::provider::AiProvider;
use crate::types::{GenerateRequest, GenerateResponse, ProviderError};

/// LM Studio（OpenAI 互換 API）プロバイダ
pub struct LmStudioProvider {
    config: LmStudioConfig,
    agent: Agent,
    /// 解決済みの認証。None は未解決
    ///
    /// 成功時だけこのインスタンスのメモリに保持する。`chat` で毎ターン secret store を
    /// 呼ばないためで、ディスクには保存せず、設定の異なるインスタンスとも共有しない。
    auth: Mutex<Option<Auth>>,
}

impl LmStudioProvider {
    #[must_use]
    pub fn new(config: LmStudioConfig) -> Self {
        Self {
            config,
            agent: client::agent(),
            auth: Mutex::new(None),
        }
    }

    /// 認証を解決する。未解決なら疎通確認の後にトークンを用意する
    fn resolve_auth(&self) -> Result<Auth, ProviderError> {
        // 同時に呼ばれても、secret store のロック解除を伴うトークンコマンドを二重に走らせない
        let mut cached = self.auth.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(auth) = cached.as_ref() {
            return Ok(auth.clone());
        }

        // サーバー停止中に secret store のロック解除を求めないよう、トークンより先に疎通を確かめる。
        // `is_available()` を経ずに `generate()` が呼ばれても、この順序を守る
        if !self.is_available() {
            return Err(ProviderError::GenerationFailed(self.unavailable_message()));
        }
        let auth = match (&self.config.token, &self.config.token_command) {
            (Some(token), _) => Auth::Bearer(token.clone()),
            (None, Some(command)) => Auth::Bearer(run_token_command(command)?),
            (None, None) => Auth::None,
        };
        *cached = Some(auth.clone());
        Ok(auth)
    }

    /// モデルキーがサーバーの一覧にあるかを確かめる
    fn ensure_model_listed(&self, auth: &Auth) -> Result<(), ProviderError> {
        let models = client::list_models(
            &self.agent,
            &self.config.base_url,
            auth,
            self.config.models_timeout,
        )?;
        if models.iter().any(|id| id == &self.config.model) {
            return Ok(());
        }
        let available = if models.is_empty() {
            "none".to_string()
        } else {
            models.join(", ")
        };
        Err(ProviderError::GenerationFailed(format!(
            "model '{}' not found on LM Studio (available: {available})",
            self.config.model
        )))
    }
}

impl AiProvider for LmStudioProvider {
    fn name(&self) -> &str {
        crate::LM_STUDIO
    }

    fn model(&self) -> Option<&str> {
        Some(&self.config.model)
    }

    fn is_available(&self) -> bool {
        client::probe(
            &self.agent,
            &self.config.base_url,
            self.config.probe_timeout,
        )
    }

    fn unavailable_message(&self) -> String {
        format!("LM Studio server not reachable at {}", self.config.base_url)
    }

    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, ProviderError> {
        // 不正なリクエストは、ネットワークや secret store に触れる前に弾く
        let body = payload::build_request_body(&self.config, request)?;
        let auth = self.resolve_auth()?;
        // LM Studio は未知のモデルキーにもロード済みの別モデルで応答してしまう。
        // チャット中にモデルが削除された場合も検出するため、生成のたびに一覧と照合する
        self.ensure_model_listed(&auth)?;
        let response = client::chat_completion(
            &self.agent,
            &self.config.base_url,
            &auth,
            &body.to_string(),
            self.config.request_timeout,
        )?;
        let content =
            payload::parse_completion(&response, &self.config, request.response_schema.is_some())?;

        Ok(GenerateResponse {
            content,
            provider: self.name().to_string(),
        })
    }
}

/// `LM_API_TOKEN_COMMAND` を実行してトークンを得る
fn run_token_command(command: &str) -> Result<String, ProviderError> {
    // stdin は閉じて入力待ちで止まらないようにし、stderr（op のエラー等）は利用者に見せる
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| {
            ProviderError::Other(format!("failed to run LM_API_TOKEN_COMMAND: {error}"))
        })?;
    if !output.status.success() {
        return Err(ProviderError::Other(format!(
            "LM_API_TOKEN_COMMAND failed ({})",
            output.status
        )));
    }

    // 出力はトークンそのものなので、どのエラー文にも含めない
    let token = String::from_utf8(output.stdout).map_err(|_| {
        ProviderError::Other("LM_API_TOKEN_COMMAND printed non-UTF-8 output".to_string())
    })?;
    // トークンは空白を含まない。前後の空白を送ると 401 になり、トークンを伏せたエラーでは原因が分からない
    let token = token.trim();
    if token.is_empty() {
        return Err(ProviderError::Other(
            "LM_API_TOKEN_COMMAND printed an empty token".to_string(),
        ));
    }
    Ok(token.to_string())
}
