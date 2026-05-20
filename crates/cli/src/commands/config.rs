#[derive(clap::Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum ConfigCommand {
    /// Show current persisted API URL config.
    Show,
    /// Set persisted API URL.
    SetApiUrl { url: String },
    /// Reset persisted API URL to default behavior.
    ResetApiUrl,
}
