use std::path::Path;

use ai_provider::provider::AiProvider;
use ai_provider::types::{FileContext, GenerateRequest, Message, Role};
use anyhow::{Context, Result};
use chrono::Utc;

/// 単発クエリを実行し、結果を���準出力に表示する
pub fn run(
    provider: &dyn AiProvider,
    prompt: &str,
    system: Option<&str>,
    files: &[&Path],
) -> Result<()> {
    let file_contexts = files
        .iter()
        .map(|path| {
            let content =
                std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
            Ok(FileContext {
                filename: path
                    .file_name()
                    .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().to_string()),
                content,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let request = GenerateRequest {
        system_prompt: system.map(String::from),
        messages: vec![Message {
            role: Role::User,
            content: prompt.to_string(),
            timestamp: Utc::now(),
        }],
        file_contexts,
    };

    let response = provider.generate(&request)?;
    println!("{}", response.content);

    Ok(())
}
