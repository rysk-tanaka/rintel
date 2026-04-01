# Rintel

Apple Intelligence（Foundation Models）を活用した AI チャットツール。
macOS 26+ のオンデバイス AI を Rust から呼び出し、CLI と GUI の両方で対話できる。

## 必要環境

- macOS 26.0+（Foundation Models 利用に必要）
- Rust（edition 2024）
- Xcode 26+（Swift toolchain）
- Node.js + pnpm 10+（GUI のみ）

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
    ai-provider/                # trait AiProvider + Apple Intelligence 実装
    ai-session/                 # セッション管理・永続化
  apps/
    cli/                        # clap ベース CLI
    gui/                        # Tauri v2 + React GUI
```

| Crate | 責務 |
| --- | --- |
| `ai-provider` | `AiProvider` trait、Apple Intelligence FFI、非 macOS スタブ |
| `ai-session` | `Session`、`SessionManager`（JSON 永続化）、TTL 管理 |
| `rintel-cli` | ask / chat / session コマンド |
| `rintel-gui` | Tauri コマンド層、React フロントエンド |

## アーキテクチャ

```text
CLI / GUI
  ↓ (依存)
ai-session  →  ai-provider  →  swift-rs FFI  →  Foundation Models
                                                  (on-device AI)
```

- `AiProvider` trait はステートレス。会話履歴は `Session` が管理し、毎回 `GenerateRequest` に含めて渡す
- Swift FFI は `DispatchSemaphore` で async → sync 変換。非メインスレッドから呼ぶこと
- セッションは `~/.config/rintel/sessions/` に JSON で保存。CLI と GUI で共有
- `#[cfg(target_os = "macos")]` で非 macOS ビルドに対応（スタブ提供）

## テスト

```bash
cargo test -p ai-provider -p ai-session
```

## ライセンス

MIT
