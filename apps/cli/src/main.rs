mod commands;

use std::path::PathBuf;

use ai_provider::AppleIntelligenceProvider;
use ai_provider::provider::AiProvider;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rintel", about = "Apple Intelligence CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Single-shot query
    Ask {
        /// The prompt to send
        prompt: String,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,

        /// Files to include as context
        #[arg(short, long, value_name = "FILE")]
        file: Vec<PathBuf>,
    },

    /// Interactive chat
    Chat {
        /// System prompt
        #[arg(short, long)]
        system: Option<String>,

        /// Files to include as context
        #[arg(short, long, value_name = "FILE")]
        file: Vec<PathBuf>,

        /// Resume an existing session (full or short UUID)
        #[arg(long)]
        resume: Option<String>,
    },

    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List saved sessions
    List,
    /// Show session details
    Show { id: String },
    /// Delete a session
    Delete { id: String },
    /// Remove expired sessions
    Cleanup,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let provider = AppleIntelligenceProvider::new();

    match &cli.command {
        Commands::Ask {
            prompt,
            system,
            file,
        } => {
            if !provider.is_available() {
                eprintln!("Warning: Apple Intelligence is not available on this system.");
            }
            let file_refs: Vec<&std::path::Path> = file.iter().map(PathBuf::as_path).collect();
            commands::ask::run(&provider, prompt, system.as_deref(), &file_refs)?;
        }
        Commands::Chat {
            system,
            file,
            resume,
        } => {
            if !provider.is_available() {
                anyhow::bail!("Apple Intelligence is not available on this system.");
            }
            let file_refs: Vec<&std::path::Path> = file.iter().map(PathBuf::as_path).collect();
            commands::chat::run(&provider, system.as_deref(), &file_refs, resume.as_deref())?;
        }
        Commands::Session { action } => {
            commands::session::run(action)?;
        }
    }

    Ok(())
}
