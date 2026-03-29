# TODO

## 完了済み

- [x] Phase 1: Foundation — `rintel ask` で Apple Intelligence 呼び出し
- [x] Phase 2: セッション永続化 + ファイルコンテキスト + TTL
- [x] Phase 3: 対話チャット REPL（`rintel chat`）
- [x] Phase 4: マルチターン（`ai_generate_with_history` FFI）
- [x] Phase 5: GUI（Tauri v2 + React）

## 未着手

- [ ] マルチバックエンド対応（OpenAI, Anthropic — `reqwest::blocking`）
- [ ] TOML 設定ファイル（`~/.config/rintel/config.toml`）
- [ ] ストリーミング応答（GUI でのインクリメンタル表示）
- [ ] セッションタイトル自動生成（最初のメッセージから要約）
- [ ] GUI: システムプロンプト設定 UI
- [ ] GUI: セッション検索・フィルタ
- [ ] CI（lint, test, build）
