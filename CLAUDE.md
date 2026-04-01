# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

Apple Intelligence（Foundation Models）を Rust から呼び出す AI チャットツール。
CLI（clap + rustyline）と GUI（Tauri v2 + React）の 2 つのフロントエンドを持つ。
`swift-rs` で Rust ↔ Swift FFI を実現し、macOS 26+ のオンデバイス AI を利用する。

## 開発コマンド

```bash
# Rust
cargo check                        # workspace 全体の型チェック
cargo clippy --locked -- -D warnings  # lint
cargo fmt --check                  # フォーマットチェック
cargo test -p ai-provider           # プロバイダテスト
cargo test -p ai-session            # セッションテスト
cargo test -p ai-session -- persistence::tests::save_and_load_roundtrip  # 特定テスト
cargo run -p rintel-cli -- ask "Hello"   # CLI 実行
cargo update-licenses              # THIRD_PARTY_LICENSES.html 再生成（要: cargo-about）

# GUI（apps/gui/ で実行）
pnpm install --ignore-workspace     # 依存インストール（親 workspace を無視）
pnpm tauri dev                      # 開発サーバー起動
pnpm tauri build --debug            # デバッグビルド
pnpm exec tsc --noEmit -p tsconfig.app.json  # TypeScript 型チェック
```

`apps/gui/` は親リポジトリの pnpm workspace 内にあるため、`--ignore-workspace` が必要。
`apps/gui/pnpm-workspace.yaml` で独立した workspace として動作する。

## アーキテクチャ

### Crate 構成

| Crate | 責務 |
| --- | --- |
| `ai-provider` | `AiProvider` trait 定義、Apple Intelligence FFI ブリッジ、非 macOS スタブ |
| `ai-session` | `Session`（会話状態）、`SessionManager`（JSON 永続化）、`SessionConfig`（TTL・保存先） |
| `rintel-cli` | clap サブコマンド（ask / chat / session）、rustyline REPL |
| `rintel-gui` | Tauri コマンド層（AppState → ai-provider/ai-session 呼び出し）、React フロントエンド |

### 通信規約

- CLI: `ai-provider` / `ai-session` の API を直接呼び出す（sync）
- GUI: Tauri IPC（`invoke()`）経由。ブロッキング FFI は `tokio::task::spawn_blocking` で実行
- Swift FFI: JSON 文字列で通信（`{"ok":"text"}` / `{"error":"type","message":"msg"}`）
- セッション保存先: `~/.config/rintel/sessions/{uuid}.json`（CLI と GUI で共有）

### プラットフォーム分離

`ai-provider` は `#[cfg(target_os = "macos")]` で Apple Intelligence 実装と非 macOS スタブを切り替える。
スタブは `is_available() → false` を返すだけで、ビルドは通る。

### SwiftLinker

`ai-provider/build.rs` の `SwiftLinker` が `cargo:rustc-link-lib` / `cargo:rustc-link-search` を emit する。
これらは依存先バイナリ（CLI / GUI）に伝播するため、各 app の build.rs には SwiftLinker 不要。
ただし rpath は伝播しないため、各 app の build.rs で xcode-select rpath を設定する。

### `with_package` の名前

`SwiftLinker::with_package("AppleIntelligence", ...)` の第1引数は SwiftPM のターゲット名。
パッケージ名（`apple-intelligence`）ではなくターゲット名（`AppleIntelligence`）を指定すること。

## Tauri コマンド（`apps/gui/src-tauri/src/commands/`）

| コマンド | 備考 |
| --- | --- |
| `check_ai_availability` | `AiProvider::is_available()` |
| `send_message` | `spawn_blocking` で `session.send()` を呼ぶ |
| `list_sessions` / `create_session` / `load_session` / `delete_session` / `cleanup_sessions` | SessionManager ラッパー |
| `add_file_context` | ファイル読み込み → セッションに追加 |
| `list_claude_projects` / `list_claude_sessions` / `get_claude_session` | Claude Code セッション閲覧（読み取り専用、AppState 不要） |

### Clippy lint 設定

`rintel-gui` は `clippy::pedantic` を有効化し、`unwrap_used` / `expect_used` を warn、`dbg_macro` を deny としている。
他の crate には lint 設定がないため、CI では `cargo clippy --locked -- -D warnings` で全体をチェックする。

## GUI フロントエンド（`apps/gui/src/`）

- `invoke()` 呼び出しは `hooks/useAi.ts`、`hooks/useSessions.ts`、`hooks/useClaudeCode.ts` に集約
- 型定義は `types.ts` で Rust struct と手動同期（コード生成なし）
- 時刻表示は絶対時刻を使用（`formatTime.ts`）
- App.tsx のタブバーで Chat / Claude Code ビューを切り替え
- Claude Code ビューアは `components/claude-code/` に格納（3ペイン構成: プロジェクト → セッション → 会話）

### Claude Code セッション閲覧

- `~/.claude/projects/` 配下の JSONL セッションファイルを読み取り専用で表示
- パスデコード: ディレクトリ名 `-Users-rysk-myapp` → `/Users/rysk/myapp`（ハイフン含むパスは誤変換の可能性あり、表示用途のみ）
- JSONL フィルタ: `type == "user" | "assistant"` のみ、`isCompactSummary` / `isSidechain` はスキップ
- コマンドは `State<AppState>` 不要（ファイルシステム読み取りのみ）
- `project_dir` / `session_id` パラメータにはパストラバーサル対策あり

## セッション

- デフォルト TTL: 1 時間
- UUID プレフィックスで短縮指定可能（`SessionManager::resolve_prefix`）
- `Session::send()` は user/assistant メッセージを追加し、`AiProvider::generate()` を呼ぶ
- マルチターン: `ai_generate_with_history` FFI で `LanguageModelSession` に会話履歴を再現

## テスト方針

- `ai-provider`: プロバイダの可用性テスト（FFI 非依存）
- `ai-session`: Session のメッセージ管理、SessionManager の CRUD・TTL テスト
- テスト内のファイルシステム操作には `tempfile` を使用
- CLI / GUI のコマンド層は E2E で担保
