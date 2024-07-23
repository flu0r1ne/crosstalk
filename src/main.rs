mod chat;
mod providers;
mod repl;

use clap::{Parser, Subcommand};
use die::die;
use providers::registry::populated_registry;

#[derive(Parser)]
#[command(name = "my-program")]
#[command(
    about = "A general-purpose CLI for chat models",
    author = "Alex <alex@al.exander.io>",
    version = "0.0.1"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Starts the chat
    Chat(ChatArgs),
}

#[derive(Parser, Default)]
struct ChatArgs {
    /// Specifies the model to be used for chat
    #[arg(short, long)]
    model: Option<String>,
}

use crate::repl::chat_repl;

async fn handle_chat(args: &ChatArgs) {
    let mut registry = populated_registry().await;

    let provider = registry.resolve(args.model.as_deref()).await;

    if let Err(err) = provider {
        die!("Failed to resovle model: {}", err);
    }

    let spec = provider.unwrap();

    chat_repl(spec.model_id, spec.provider).await;
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Chat(args)) => handle_chat(args).await,
        None => handle_chat(&ChatArgs::default()).await,
    }
}
