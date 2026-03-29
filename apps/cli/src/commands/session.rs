use ai_session::{SessionConfig, SessionManager};
use anyhow::Result;
use chrono::Local;

use crate::SessionAction;

pub fn run(action: &SessionAction) -> Result<()> {
    let config = SessionConfig::default();
    let manager = SessionManager::new(&config.storage_dir)?;

    match action {
        SessionAction::List => list(&manager),
        SessionAction::Show { id } => show(&manager, id),
        SessionAction::Delete { id } => delete(&manager, id),
        SessionAction::Cleanup => cleanup(&manager),
    }
}

fn list(manager: &SessionManager) -> Result<()> {
    let sessions = manager.list()?;

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    for s in &sessions {
        let title = s.title.as_deref().unwrap_or("(untitled)");
        let expired = if s.expired { " [expired]" } else { "" };
        let time = s.last_active.with_timezone(&Local).format("%Y-%m-%d %H:%M");
        println!(
            "{id}  {title}  ({msgs} msgs, {time}){expired}",
            id = &s.id.to_string()[..8],
            msgs = s.message_count,
        );
    }

    Ok(())
}

fn show(manager: &SessionManager, prefix: &str) -> Result<()> {
    let id = manager.resolve_prefix(prefix)?;
    let session = manager.load(&id)?;

    println!("Session: {}", session.id);
    if let Some(title) = &session.title {
        println!("Title:   {title}");
    }
    println!("Created: {}", session.created_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"));
    println!("Active:  {}", session.last_active.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"));
    if let Some(ttl) = session.ttl_secs {
        println!("TTL:     {ttl}s");
    }
    if let Some(system) = &session.system_prompt {
        println!("System:  {system}");
    }
    if !session.file_contexts.is_empty() {
        println!("Files:   {}", session.file_contexts.iter().map(|f| f.filename.as_str()).collect::<Vec<_>>().join(", "));
    }
    println!("---");

    for msg in &session.messages {
        let label = match msg.role {
            ai_provider::types::Role::User => "You",
            ai_provider::types::Role::Assistant => "AI",
        };
        println!("{label}: {}", msg.content);
        println!();
    }

    Ok(())
}

fn delete(manager: &SessionManager, prefix: &str) -> Result<()> {
    let id = manager.resolve_prefix(prefix)?;
    manager.delete(&id)?;
    println!("Deleted session {id}");
    Ok(())
}

fn cleanup(manager: &SessionManager) -> Result<()> {
    let count = manager.cleanup_expired()?;
    println!("Removed {count} expired session(s).");
    Ok(())
}
