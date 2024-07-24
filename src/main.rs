mod chat;
mod cli;
mod providers;

use clap::{Parser, Subcommand};
use cli::chat::chat_cmd;

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
pub(crate) struct ChatArgs {
    /// Specifies the model to be used during the chat
    #[arg(short, long)]
    model: Option<String>,
    /// Enter interactive mode
    #[arg(short, long)]
    interactive: bool,
    /// Optionally specify the initial prompt
    prompt: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Chat(args)) => chat_cmd(args).await,
        None => chat_cmd(&ChatArgs::default()).await,
    }
}
