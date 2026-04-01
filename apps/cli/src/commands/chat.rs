use std::path::Path;

use ai_provider::provider::AiProvider;
use ai_session::{Session, SessionConfig, SessionManager};
use anyhow::Result;
use colored::Colorize;
use rustyline::DefaultEditor;

/// 対話チャット REPL を開始する
pub fn run(
    provider: &dyn AiProvider,
    system: Option<&str>,
    files: &[&Path],
    resume: Option<&str>,
) -> Result<()> {
    let config = SessionConfig::default();
    let manager = SessionManager::new(&config.storage_dir)?;

    let mut session = if let Some(prefix) = resume {
        let id = manager.resolve_prefix(prefix)?;
        let s = manager.load(&id)?;
        if s.is_expired() {
            anyhow::bail!("session {} has expired", s.id);
        }
        eprintln!(
            "{}",
            format!(
                "Resumed session {} ({} messages)",
                &s.id.to_string()[..8],
                s.messages.len()
            )
            .dimmed()
        );

        // 過去の会話内容を表示
        if !s.messages.is_empty() {
            eprintln!("{}", "--- Previous conversation ---".dimmed());
            for msg in &s.messages {
                let (label, color_fn): (&str, fn(&str) -> colored::ColoredString) = match msg.role {
                    ai_provider::types::Role::User => ("You", |s| s.bold()),
                    ai_provider::types::Role::Assistant => ("AI", |s| s.cyan()),
                };
                eprintln!("{}  {}", color_fn(label), msg.content.dimmed());
            }
            eprintln!("{}", "-----------------------------".dimmed());
        }

        s
    } else {
        let ttl_secs = config.default_ttl.map(|d| d.as_secs());
        let mut s = Session::new(system.map(String::from), ttl_secs);

        for path in files {
            s.add_file_context(path)?;
        }

        s
    };

    eprintln!(
        "{}",
        format!(
            "Session: {} | Type /help for commands, /quit to exit",
            &session.id.to_string()[..8]
        )
        .dimmed()
    );

    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline(&format!("{} ", "You:".bold()));

        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed.starts_with('/') {
                    match handle_command(trimmed, &session, &manager, files) {
                        CommandResult::Continue => continue,
                        CommandResult::Quit => {
                            manager.save(&session)?;
                            eprintln!("{}", "Session saved.".dimmed());
                            break;
                        }
                        CommandResult::Clear => {
                            session.messages.clear();
                            eprintln!("{}", "History cleared.".dimmed());
                            continue;
                        }
                    }
                }

                let _ = rl.add_history_entry(&line);

                match session.send(provider, trimmed) {
                    Ok(response) => {
                        println!("\n{}  {}\n", "AI:".bold().cyan(), response);
                    }
                    Err(e) => {
                        eprintln!("{} {e}", "Error:".red());
                    }
                }

                // Auto-save after each exchange
                if let Err(e) = manager.save(&session) {
                    eprintln!("{} {e}", "Warning: failed to save session:".yellow());
                }
            }
            Err(
                rustyline::error::ReadlineError::Interrupted | rustyline::error::ReadlineError::Eof,
            ) => {
                manager.save(&session)?;
                eprintln!("\n{}", "Session saved.".dimmed());
                break;
            }
            Err(e) => {
                anyhow::bail!("readline error: {e}");
            }
        }
    }

    Ok(())
}

enum CommandResult {
    Continue,
    Quit,
    Clear,
}

fn handle_command(
    cmd: &str,
    session: &Session,
    _manager: &SessionManager,
    files: &[&Path],
) -> CommandResult {
    match cmd {
        "/quit" | "/exit" | "/q" => CommandResult::Quit,
        "/clear" => CommandResult::Clear,
        "/help" | "/h" => {
            eprintln!("{}", "Commands:".bold());
            eprintln!("  /quit    Exit and save session");
            eprintln!("  /clear   Clear conversation history");
            eprintln!("  /files   Show loaded file contexts");
            eprintln!("  /info    Show session info");
            eprintln!("  /help    Show this help");
            CommandResult::Continue
        }
        "/files" => {
            if session.file_contexts.is_empty() && files.is_empty() {
                eprintln!("{}", "No files loaded.".dimmed());
            } else {
                for fc in &session.file_contexts {
                    eprintln!("  {} ({} bytes)", fc.filename, fc.content.len());
                }
            }
            CommandResult::Continue
        }
        "/info" => {
            eprintln!("  Session:  {}", session.id);
            eprintln!("  Messages: {}", session.messages.len());
            if let Some(system) = &session.system_prompt {
                eprintln!("  System:   {system}");
            }
            if let Some(ttl) = session.ttl_secs {
                eprintln!("  TTL:      {ttl}s");
            }
            CommandResult::Continue
        }
        _ => {
            eprintln!(
                "{}",
                format!("Unknown command: {cmd}. Type /help for commands.").yellow()
            );
            CommandResult::Continue
        }
    }
}
