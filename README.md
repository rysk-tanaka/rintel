# Rintel

[![lint](https://github.com/rysk-tanaka/rintel/actions/workflows/lint.yml/badge.svg)](https://github.com/rysk-tanaka/rintel/actions/workflows/lint.yml)
[![test](https://github.com/rysk-tanaka/rintel/actions/workflows/test.yml/badge.svg)](https://github.com/rysk-tanaka/rintel/actions/workflows/test.yml)
[![build](https://github.com/rysk-tanaka/rintel/actions/workflows/build.yml/badge.svg)](https://github.com/rysk-tanaka/rintel/actions/workflows/build.yml)
[![release](https://img.shields.io/github/v/release/rysk-tanaka/rintel)](https://github.com/rysk-tanaka/rintel/releases/latest)
[![license](https://badgers.space/github/license/rysk-tanaka/rintel?corner_radius=5)](./LICENSE)

Apple Intelligence（Foundation Models）を活用した AI チャットツール。
macOS 26+ のオンデバイス AI を Rust から呼び出し、CLI と GUI の両方で対話できる。
CLI では [LM Studio](https://lmstudio.ai/) で動くローカル LLM も選べる。

## 必要環境

- macOS 26.0+（Foundation Models 利用に必要）
- Rust（edition 2024）
- Xcode 26+（Swift toolchain）
- Node.js + pnpm 10+（GUI のみ）
- LM Studio（任意。CLI で `--provider lm-studio` を使う場合のみ）

## ビルド

```bash
# CLI
cargo build -p rintel-cli

# GUI
cd apps/gui && pnpm install --ignore-workspace && pnpm tauri build --debug
```

## CLI 使い方

```bash
# 単発クエリ
rintel ask "Rustの所有権について説明して"
rintel ask -s "技術ライターとして回答して" "この概念を初心者向けに"
rintel ask -f src/main.rs "このコードをレビューして"

# 対話チャット
rintel chat
rintel chat -s "日本語で回答してください"
rintel chat --resume 0d9b2181    # セッション再開（UUID プレフィックス可）

# セッション管理
rintel session list
rintel session show 0d9b2181
rintel session delete 0d9b2181
rintel session cleanup            # 期限切れセッション削除
```

チャット内コマンド: `/quit`, `/clear`, `/files`, `/info`, `/help`

### LM Studio

`ask` / `chat` に `--provider lm-studio` を付けると、LM Studio の OpenAI 互換 API でローカル LLM を使う。
何も指定しなければ従来どおり Apple Intelligence を使う。

```bash
rintel ask --provider lm-studio "Rustの所有権について説明して"
rintel ask --provider lm-studio --model qwen/qwen3.6-35b-a3b -f src/main.rs "このコードをレビューして"
RINTEL_PROVIDER=lm-studio rintel chat
```

接続設定（`LM_API_*`）は [rysk-tanaka/skills](https://github.com/rysk-tanaka/skills) の LM Studio 系 skill と共通。

| 変数 | 用途 | 既定値 |
| --- | --- | --- |
| `RINTEL_PROVIDER` | `--provider` 省略時のプロバイダ（`apple` / `lm-studio`） | `apple` |
| `LM_API_URL` | LM Studio サーバーのルート URL（`/v1` は付けない。`http://` のみ対応） | `http://localhost:1234` |
| `LM_API_TOKEN` | 認証トークン | なし |
| `LM_API_TOKEN_COMMAND` | トークンを標準出力に出すコマンド（例: `op read 'op://...'`）。`LM_API_TOKEN` が空のとき、接続確認の後にだけ実行される | なし |
| `RINTEL_LMS_MODEL` | `--model` 省略時のモデルキー（`lms ls` で確認） | `qwen/qwen3.6-35b-a3b` |
| `RINTEL_LMS_THINKING` | `true` で思考させる | `false` |
| `RINTEL_LMS_MAX_TOKENS` | 出力トークンの上限（思考を含む） | `16384` |
| `RINTEL_LMS_TIMEOUT` | 生成リクエストのタイムアウト秒数（上限 `86400`） | `300` |
| `RINTEL_LMS_TTL` | JIT ロードしたモデルを最後のリクエストからアンロードするまでの秒数 | `600` |

- トークン関連の変数が両方とも空なら、認証なしで接続する
- `HTTP_PROXY` などプロキシの環境変数は使わず、LM Studio へ直接接続する
- トークンはプロセスのメモリにだけ保持する。`ask` は実行ごとに、`chat` は最初の送信時にだけ `LM_API_TOKEN_COMMAND` を実行する
- LM Studio は未知のキーや埋め込みモデルのキーにもロード済みの別モデルで応答する。そのため生成のたびにモデルキーをサーバーのモデル一覧と照合し、応答したモデルも確かめる
- セッションには作成時のプロバイダとモデルが記録される。`chat --resume` は記録どおりに再開し、環境変数は使わない。異なる `--provider` / `--model` を明示するとエラーになる
- GUI は Apple Intelligence のみ対応。LM Studio で作ったセッションは GUI から送信できない

## GUI

Tauri v2 + React のデスクトップアプリ。セッション一覧・チャット・ファイルコンテキスト追加に加え、Claude Code のセッション履歴閲覧が可能。

```bash
cd apps/gui && pnpm tauri dev
```

## プロジェクト構成

```tree
rintel/
  Cargo.toml                    # workspace root
  apple-intelligence/           # SwiftPM パッケージ（Rust ↔ Swift FFI）
  crates/
    ai-provider/                # trait AiProvider + Apple Intelligence / LM Studio 実装
    ai-session/                 # セッション管理・永続化
  apps/
    cli/                        # clap ベース CLI
    gui/                        # Tauri v2 + React GUI
```

| Crate | 責務 |
| --- | --- |
| `ai-provider` | `AiProvider` trait、Apple Intelligence FFI、LM Studio クライアント、プロバイダ選択、非 macOS スタブ |
| `ai-session` | `Session`、`SessionManager`（JSON 永続化）、TTL 管理 |
| `rintel-cli` | ask / chat / session コマンド |
| `rintel-gui` | Tauri コマンド層、React フロントエンド |

## アーキテクチャ

```text
CLI / GUI
  ↓ (依存)
ai-session  →  ai-provider  →  swift-rs FFI  →  Foundation Models
                    │                             (on-device AI)
                    └─────→  HTTP (ureq)  →  LM Studio
                                              (OpenAI 互換 API)
```

- `AiProvider` trait はステートレス。会話履歴は `Session` が管理し、毎回 `GenerateRequest` に含めて渡す
- Swift FFI は `DispatchSemaphore` で async → sync 変換。非メインスレッドから呼ぶこと
- LM Studio は同期 HTTP クライアント（ureq）で呼ぶ。TLS は組み込んでいない
- セッションは `~/.config/rintel/sessions/` に JSON で保存。CLI と GUI で共有
- `#[cfg(target_os = "macos")]` で非 macOS ビルドに対応（スタブ提供）

## テスト

```bash
cargo test -p ai-provider -p ai-session
```

## ライセンス

MIT License — © Ryosuke Tanaka

サードパーティのライセンス情報は [THIRD_PARTY_LICENSES.html](./THIRD_PARTY_LICENSES.html) を参照してください。
