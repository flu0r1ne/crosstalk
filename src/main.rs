mod chat;
mod cli;
mod config;
mod providers;
mod registry;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use cli::{chat::chat_cmd, list::list_cmd, ColorMode};
use config::read_config;
use providers::providers::ProviderIdentifier;
use registry::populate::populated_registry;

#[derive(
    Parser, Default, Clone, Copy, ValueEnum, strum_macros::Display, strum_macros::EnumString,
)]
#[strum(serialize_all = "lowercase")]
pub(crate) enum RequestedColorMode {
    #[default]
    Auto,
    On,
    Off,
}

#[derive(Parser)]
#[command(name = "crosstalk")]
#[command(
    about = "A general-purpose CLI for chat models",
    author = "Alex <alex@al.exander.io>",
    version = "0.0.1-alpha.0"
)]
struct Cli {
    #[arg(long, default_value_t = RequestedColorMode::default())]
    color: RequestedColorMode,
    #[arg(long)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a chat
    Chat(ChatArgs),
    /// List available models
    List(ListArgs),
}

#[derive(Parser, Default)]
pub(crate) struct ChatArgs {
    /// Specifies the model to be used during the chat
    #[arg(short, long)]
    model: Option<String>,
    /// Enter interactive mode
    #[arg(short, long)]
    interactive: bool,
    /// Specify the initial prompt
    prompt: Option<String>,
}

/// Possible listings
#[derive(Subcommand)]
pub(crate) enum ListObject {
    /// Registered models
    Models(ListModelArgs),
    /// Providers
    Providers,
}

/// Output formats
#[derive(
    Parser, ValueEnum, Default, Clone, Copy, strum_macros::Display, strum_macros::EnumString,
)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum ListingFormat {
    /// Format the output as a table
    #[default]
    Table,
    /// Format the output as JSON
    Json,
    /// Format the output as a table without a header
    HeaderlessTable,
}

#[derive(Parser)]
pub(crate) struct ListArgs {
    /// Output the listing with the specified format
    #[arg(short, long, default_value_t = ListingFormat::default())]
    format: ListingFormat,
    /// List the specified object
    #[command(subcommand)]
    object: ListObject,
}

#[derive(Parser, Default)]
pub(crate) struct ListModelArgs {
    /// Limit listing to the specified provider
    #[arg(short, long)]
    provider: Option<ProviderIdentifier>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let color = ColorMode::resolve_auto(cli.color);

    let config = read_config(cli.config);

    let registry = populated_registry(&config).await;

    let editor: Option<PathBuf> = config.editor.map(|s| s.into());

    match &cli.command {
        Some(Commands::Chat(args)) => chat_cmd(editor, config.default_model, registry, args).await,
        Some(Commands::List(args)) => list_cmd(color, registry, args).await,
        None => chat_cmd(editor, config.default_model, registry, &ChatArgs::default()).await,
    }
}
