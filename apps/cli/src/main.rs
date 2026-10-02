mod commands;

use std::path::PathBuf;

use ai_provider::lmstudio::DEFAULT_MODEL;
use ai_provider::provider::AiProvider;
use ai_provider::{ProviderKind, ProviderSpec};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "rintel",
    about = "Local AI chat CLI (Apple Intelligence / LM Studio)"
)]
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

        /// JSON Schema file for guided/structured output (returns conforming JSON)
        #[arg(long, value_name = "FILE")]
        schema: Option<PathBuf>,

        #[command(flatten)]
        provider: ProviderArgs,
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

        #[command(flatten)]
        provider: ProviderArgs,
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

/// `ask` / `chat` のプロバイダ指定
///
/// 環境変数（`RINTEL_PROVIDER` 等）は clap の `env` で読まない。`session` サブコマンドが
/// 不正な環境変数の影響を受けないよう、`ask` / `chat` の実行時にだけ解釈する。
#[derive(Args)]
pub struct ProviderArgs {
    /// AI provider [default: $RINTEL_PROVIDER, then apple]
    #[arg(long, value_enum)]
    provider: Option<ProviderArg>,

    #[arg(
        long,
        value_name = "KEY",
        help = format!("LM Studio model key [default: $RINTEL_LMS_MODEL, then {DEFAULT_MODEL}]")
    )]
    model: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum ProviderArg {
    /// Apple Intelligence (on-device Foundation Models)
    Apple,
    /// LM Studio (OpenAI-compatible API, configured via LM_API_URL)
    LmStudio,
}

impl ProviderArgs {
    /// プロバイダを解決・構築し、利用可能であることを確かめる
    ///
    /// `recorded` は再開するセッションに記録された (プロバイダ名, モデル)。
    pub fn connect(
        &self,
        recorded: Option<(&str, Option<&str>)>,
    ) -> anyhow::Result<Box<dyn AiProvider>> {
        let kind = self.provider.map(|arg| match arg {
            ProviderArg::Apple => ProviderKind::Apple,
            ProviderArg::LmStudio => ProviderKind::LmStudio,
        });
        let spec = ProviderSpec::resolve(kind, self.model.as_deref(), recorded, env_lookup)?;
        let provider = spec.build(env_lookup)?;
        if !provider.is_available() {
            anyhow::bail!("{}", provider.unavailable_message());
        }
        Ok(provider)
    }
}

fn env_lookup(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Ask {
            prompt,
            system,
            file,
            schema,
            provider,
        } => {
            let provider = provider.connect(None)?;
            let file_refs: Vec<&std::path::Path> = file.iter().map(PathBuf::as_path).collect();
            commands::ask::run(
                provider.as_ref(),
                prompt,
                system.as_deref(),
                &file_refs,
                schema.as_deref(),
            )?;
        }
        Commands::Chat {
            system,
            file,
            resume,
            provider,
        } => {
            let file_refs: Vec<&std::path::Path> = file.iter().map(PathBuf::as_path).collect();
            commands::chat::run(provider, system.as_deref(), &file_refs, resume.as_deref())?;
        }
        Commands::Session { action } => {
            commands::session::run(action)?;
        }
    }

    Ok(())
}
