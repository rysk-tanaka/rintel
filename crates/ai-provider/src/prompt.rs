//! プロバイダ共通のプロンプト組み立て

use crate::types::{FileContext, Message, Role};

/// ファイルコンテキストをプレフィックス文字列に変換する
pub(crate) fn build_file_prefix(files: &[FileContext]) -> String {
    if files.is_empty() {
        return String::new();
    }

    let mut prefix = String::from("--- Reference Files ---\n\n");
    for file in files {
        use std::fmt::Write;
        let _ = writeln!(
            prefix,
            "### {}\n```\n{}\n```\n",
            file.filename, file.content
        );
    }
    prefix.push_str("--- End of Files ---\n\n");
    prefix
}

/// シングルターン用のプロンプト構築
// 非 macOS では Apple Intelligence のスタブしかなく、呼び出し元が存在しない
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn build_single_prompt(messages: &[Message], files: &[FileContext]) -> String {
    let file_prefix = build_file_prefix(files);

    if messages.len() == 1 && messages[0].role == Role::User {
        return format!("{file_prefix}{}", messages[0].content);
    }

    let mut prompt = file_prefix;
    for msg in messages {
        let prefix = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        prompt.push_str(&format!("{prefix}: {}\n\n", msg.content));
    }
    prompt
}
