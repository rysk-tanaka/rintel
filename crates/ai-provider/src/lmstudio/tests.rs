//! モック HTTP サーバーを相手にした `LmStudioProvider` の結合テスト
//!
//! 実際の LM Studio や secret store（1Password 等）は呼ばない。トークンコマンドには
//! `printf` と、実行されたかを記録するマーカーファイルを使う。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::{Value, json};

use super::config::LmStudioConfig;
use super::provider::LmStudioProvider;
use crate::provider::AiProvider;
use crate::types::{GenerateRequest, Message, Role};

const MODEL: &str = "test/model";

/// テスト中の HTTP 呼び出しの上限。応答しないモックはこれより長く待たせる
const TEST_TIMEOUT: Duration = Duration::from_millis(500);
const HANG: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
struct Recorded {
    method: String,
    path: String,
    authorization: Option<String>,
    body: String,
}

enum Reply {
    Json(u16, Value),
    /// 応答ヘッダーを返さない
    Hang,
    /// ヘッダーだけ返して本文を送らない
    HeadersOnly,
    /// Content-Length より短い本文を送って接続を閉じる
    TruncatedBody(u16),
}

type Handler = dyn Fn(&Recorded) -> Reply + Send + Sync;

struct MockServer {
    api_url: String,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl MockServer {
    fn start(handler: impl Fn(&Recorded) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let api_url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let handler: Arc<Handler> = Arc::new(handler);

        let recorded = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let handler = Arc::clone(&handler);
                let recorded = Arc::clone(&recorded);
                // 応答しない接続が他のリクエストを塞がないよう、接続ごとにスレッドを分ける
                thread::spawn(move || serve(stream, handler.as_ref(), &recorded));
            }
        });

        Self { api_url, requests }
    }

    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    fn requests_to(&self, path: &str) -> Vec<Recorded> {
        self.requests()
            .into_iter()
            .filter(|request| request.path == path)
            .collect()
    }
}

fn serve(stream: TcpStream, handler: &Handler, recorded: &Mutex<Vec<Recorded>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0;
    let mut authorization = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            match name.to_ascii_lowercase().as_str() {
                "content-length" => content_length = value.trim().parse().unwrap_or(0),
                "authorization" => authorization = Some(value.trim().to_string()),
                _ => {}
            }
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();

    let request = Recorded {
        method,
        path,
        authorization,
        body: String::from_utf8(body).unwrap(),
    };
    recorded.lock().unwrap().push(request.clone());

    let mut stream = stream;
    match handler(&request) {
        Reply::Json(status, value) => {
            let body = value.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
        Reply::Hang => thread::sleep(HANG),
        Reply::TruncatedBody(status) => {
            let _ = write!(
                stream,
                "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{{\"error\""
            );
        }
        Reply::HeadersOnly => {
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.flush();
            thread::sleep(HANG);
        }
    }
}

fn completion(content: &str) -> Value {
    json!({
        "model": MODEL,
        "choices": [{
            "message": {"role": "assistant", "content": content},
            "finish_reason": "stop",
        }]
    })
}

/// モデル一覧と生成に応答する LM Studio のモック
fn lm_studio(
    models: &[&str],
    content: &str,
) -> impl Fn(&Recorded) -> Reply + Send + Sync + 'static {
    let models: Vec<Value> = models.iter().map(|id| json!({"id": id})).collect();
    let content = content.to_string();
    move |request| match request.path.as_str() {
        "/v1/models" => Reply::Json(200, json!({"data": models})),
        "/v1/chat/completions" => Reply::Json(200, completion(&content)),
        _ => Reply::Json(404, json!({"error": "not found"})),
    }
}

fn provider(api_url: &str, env: &[(&str, &str)]) -> LmStudioProvider {
    let mut env: HashMap<String, String> = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    env.insert("LM_API_URL".to_string(), api_url.to_string());

    let mut config =
        LmStudioConfig::from_lookup(MODEL.to_string(), |key| env.get(key).cloned()).unwrap();
    config.probe_timeout = TEST_TIMEOUT;
    config.models_timeout = TEST_TIMEOUT;
    config.request_timeout = TEST_TIMEOUT;
    LmStudioProvider::new(config)
}

fn ask(text: &str) -> GenerateRequest {
    GenerateRequest {
        system_prompt: None,
        messages: vec![Message {
            role: Role::User,
            content: text.to_string(),
            timestamp: Utc::now(),
        }],
        file_contexts: Vec::new(),
        response_schema: None,
    }
}

/// 実行されるたびに `marker` へ 1 文字追記し、`token` を出力するコマンド
fn counting_command(marker: &Path, token: &str) -> String {
    format!("printf x >> '{}' && printf '{token}'", marker.display())
}

fn run_count(marker: &Path) -> usize {
    std::fs::read_to_string(marker).map_or(0, |text| text.len())
}

/// 接続を受け付けないアドレス。bind して即座に閉じたポートを使う
fn unreachable_api_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{address}")
}

#[test]
fn generates_with_token_from_command_after_probe() {
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(
        &server.api_url,
        &[("LM_API_TOKEN_COMMAND", "printf test-token")],
    );

    let response = provider.generate(&ask("hi")).unwrap();
    assert_eq!(response.content, "hello");
    assert_eq!(response.provider, "lm-studio");

    let requests = server.requests();
    let summary: Vec<_> = requests
        .iter()
        .map(|r| {
            (
                r.method.as_str(),
                r.path.as_str(),
                r.authorization.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            // 疎通確認はトークンを送らない
            ("GET", "/v1/models", None),
            ("GET", "/v1/models", Some("Bearer test-token")),
            ("POST", "/v1/chat/completions", Some("Bearer test-token")),
        ]
    );
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["reasoning_effort"], "none");
    assert_eq!(body["messages"], json!([{"role": "user", "content": "hi"}]));
}

#[test]
fn direct_token_skips_command() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(
        &server.api_url,
        &[
            ("LM_API_TOKEN", "direct-token"),
            ("LM_API_TOKEN_COMMAND", &counting_command(&marker, "unused")),
        ],
    );

    provider.generate(&ask("hi")).unwrap();
    assert_eq!(run_count(&marker), 0);
    let chat = server.requests_to("/v1/chat/completions");
    assert_eq!(
        chat[0].authorization.as_deref(),
        Some("Bearer direct-token")
    );
}

#[test]
fn no_token_settings_send_no_authorization() {
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(&server.api_url, &[]);

    provider.generate(&ask("hi")).unwrap();
    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|r| r.authorization.is_none()));
}

#[test]
fn token_is_resolved_once_per_instance() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(
        &server.api_url,
        &[("LM_API_TOKEN_COMMAND", &counting_command(&marker, "cached"))],
    );

    provider.generate(&ask("first")).unwrap();
    provider.generate(&ask("second")).unwrap();
    assert_eq!(run_count(&marker), 1);
    // 疎通確認も認証の解決時だけ
    let unauthenticated = server
        .requests()
        .into_iter()
        .filter(|r| r.authorization.is_none())
        .count();
    assert_eq!(unauthenticated, 1);
}

#[test]
fn instances_do_not_share_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let marker_a = dir.path().join("a");
    let marker_b = dir.path().join("b");
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider_a = provider(
        &server.api_url,
        &[(
            "LM_API_TOKEN_COMMAND",
            &counting_command(&marker_a, "token-a"),
        )],
    );
    let provider_b = provider(
        &server.api_url,
        &[(
            "LM_API_TOKEN_COMMAND",
            &counting_command(&marker_b, "token-b"),
        )],
    );

    for provider in [&provider_a, &provider_b, &provider_a, &provider_b] {
        provider.generate(&ask("hi")).unwrap();
    }

    let tokens: Vec<_> = server
        .requests_to("/v1/chat/completions")
        .into_iter()
        .map(|r| r.authorization.unwrap())
        .collect();
    assert_eq!(
        tokens,
        [
            "Bearer token-a",
            "Bearer token-b",
            "Bearer token-a",
            "Bearer token-b"
        ]
    );
    assert_eq!(run_count(&marker_a), 1);
    assert_eq!(run_count(&marker_b), 1);
}

#[test]
fn failed_token_command_is_retried_on_next_generate() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let command = format!("printf x >> '{}'; exit 1", marker.display());
    let provider = provider(&server.api_url, &[("LM_API_TOKEN_COMMAND", &command)]);

    for _ in 0..2 {
        let error = provider.generate(&ask("hi")).unwrap_err();
        assert!(error.to_string().contains("LM_API_TOKEN_COMMAND failed"));
    }
    assert_eq!(run_count(&marker), 2);
    assert!(server.requests_to("/v1/chat/completions").is_empty());
}

#[test]
fn token_output_is_trimmed() {
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(
        &server.api_url,
        &[("LM_API_TOKEN_COMMAND", "printf '  test-token \\t\\n'")],
    );

    provider.generate(&ask("hi")).unwrap();
    let completions = server.requests_to("/v1/chat/completions");
    assert_eq!(
        completions[0].authorization.as_deref(),
        Some("Bearer test-token")
    );
}

#[test]
fn empty_token_output_is_an_error() {
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(
        &server.api_url,
        &[("LM_API_TOKEN_COMMAND", "printf ' \\n'")],
    );

    let error = provider.generate(&ask("hi")).unwrap_err();
    assert!(error.to_string().contains("empty token"));
}

#[test]
fn unreachable_server_skips_token_command() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let provider = provider(
        &unreachable_api_url(),
        &[("LM_API_TOKEN_COMMAND", &counting_command(&marker, "unused"))],
    );

    // `is_available()` を経ずに `generate()` を呼んでも、疎通確認が先に走る
    let error = provider.generate(&ask("hi")).unwrap_err();
    assert!(error.to_string().contains("not reachable"));
    assert_eq!(run_count(&marker), 0);
    assert!(!provider.is_available());
}

#[test]
fn invalid_request_fails_before_network() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    let server = MockServer::start(lm_studio(&[MODEL], "hello"));
    let provider = provider(
        &server.api_url,
        &[("LM_API_TOKEN_COMMAND", &counting_command(&marker, "unused"))],
    );
    let mut request = ask("hi");
    request.response_schema = Some("{not json".to_string());

    assert!(provider.generate(&request).is_err());
    assert_eq!(run_count(&marker), 0);
    assert!(server.requests().is_empty());
}

#[test]
fn unknown_model_is_rejected() {
    let server = MockServer::start(lm_studio(&["other/model"], "hello"));
    let provider = provider(&server.api_url, &[]);

    let error = provider.generate(&ask("hi")).unwrap_err().to_string();
    assert!(error.contains("model 'test/model' not found"), "{error}");
    assert!(error.contains("other/model"), "{error}");
    assert!(server.requests_to("/v1/chat/completions").is_empty());
}

#[test]
fn model_removed_after_first_generate_is_rejected() {
    let completions = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&completions);
    let server = MockServer::start(move |request| match request.path.as_str() {
        "/v1/models" => {
            let id = if counter.load(Ordering::SeqCst) == 0 {
                MODEL
            } else {
                "other/model"
            };
            Reply::Json(200, json!({"data": [{"id": id}]}))
        }
        _ => {
            counter.fetch_add(1, Ordering::SeqCst);
            Reply::Json(200, completion("hello"))
        }
    });
    let provider = provider(&server.api_url, &[]);

    provider.generate(&ask("first")).unwrap();
    let error = provider.generate(&ask("second")).unwrap_err();
    assert!(error.to_string().contains("not found"));
    assert_eq!(completions.load(Ordering::SeqCst), 1);
}

#[test]
fn answer_from_other_model_is_rejected() {
    // 埋め込みモデルを指定すると、LM Studio はロード済みの別モデルで応答する
    let server = MockServer::start(|request| match request.path.as_str() {
        "/v1/models" => Reply::Json(200, json!({"data": [{"id": MODEL}, {"id": "loaded/llm"}]})),
        _ => {
            let mut response = completion("hello");
            response["model"] = json!("loaded/llm");
            Reply::Json(200, response)
        }
    });
    let provider = provider(&server.api_url, &[]);

    let error = provider.generate(&ask("hi")).unwrap_err().to_string();
    assert!(
        error.contains("'loaded/llm' instead of 'test/model'"),
        "{error}"
    );
}

#[test]
fn http_error_body_is_reported() {
    let server = MockServer::start(|request| match request.path.as_str() {
        "/v1/models" => Reply::Json(200, json!({"data": [{"id": MODEL}]})),
        _ => Reply::Json(400, json!({"error": "Model is not loaded"})),
    });
    let provider = provider(&server.api_url, &[]);

    let error = provider.generate(&ask("hi")).unwrap_err().to_string();
    assert!(error.contains("HTTP 400"), "{error}");
    assert!(error.contains("Model is not loaded"), "{error}");
}

#[test]
fn http_status_is_reported_when_error_body_is_cut_off() {
    let server = MockServer::start(|_| Reply::TruncatedBody(401));
    let provider = provider(&server.api_url, &[]);

    let error = provider.generate(&ask("hi")).unwrap_err().to_string();
    assert!(error.contains("HTTP 401"), "{error}");
    assert!(error.contains("failed to read response body"), "{error}");
}

/// 応答しない段階を変えて、タイムアウトが本文の受信まで効くことを確かめる
#[derive(Clone, Copy)]
enum Stall {
    ModelsHeaders,
    ModelsBody,
    CompletionHeaders,
    CompletionBody,
}

#[test]
fn stalled_responses_time_out() {
    for stall in [
        Stall::ModelsHeaders,
        Stall::ModelsBody,
        Stall::CompletionHeaders,
        Stall::CompletionBody,
    ] {
        // トークンなしの疎通確認には応答し、認証付きのリクエストだけを止める
        let server = MockServer::start(move |request| {
            if request.authorization.is_none() {
                return Reply::Json(401, json!({"error": "unauthorized"}));
            }
            match (stall, request.path.as_str()) {
                (Stall::ModelsHeaders, "/v1/models") => Reply::Hang,
                (Stall::ModelsBody, "/v1/models") => Reply::HeadersOnly,
                (_, "/v1/models") => Reply::Json(200, json!({"data": [{"id": MODEL}]})),
                (Stall::CompletionHeaders, _) => Reply::Hang,
                _ => Reply::HeadersOnly,
            }
        });
        let provider = provider(&server.api_url, &[("LM_API_TOKEN", "t")]);
        let path = match stall {
            Stall::ModelsHeaders | Stall::ModelsBody => "/v1/models",
            Stall::CompletionHeaders | Stall::CompletionBody => "/v1/chat/completions",
        };

        let started = Instant::now();
        let error = provider.generate(&ask("hi")).unwrap_err().to_string();
        let elapsed = started.elapsed();

        // モックが HANG 後に接続を閉じるより前に、タイムアウトとして失敗すること
        assert!(error.contains(path), "{path}: {error}");
        assert!(error.contains("timeout"), "{path}: {error}");
        assert!(elapsed < HANG / 2, "{path}: took {elapsed:?}");
    }
}
