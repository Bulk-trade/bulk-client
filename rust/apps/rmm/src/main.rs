use clap::Parser;
use tokio::signal;
use tracing::info;
use crate::parts::config::RmmConfig;
use crate::parts::mm::RandomMarketMaker;

mod parts;

// ═══════════════════════════════════════════════════════════════════════════
// CLI
// ═══════════════════════════════════════════════════════════════════════════

/// OU-Process Random Market Maker (performance testing)
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Path to JSON5 config file
    #[arg(long)]
    config: String,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry point
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log_level.clone().into()),
        )
        .init();

    let config = RmmConfig::load(&cli.config)?;

    let private_key = std::env::var("BULK_PRIVATE_KEY")
        .map_err(|_| eyre::eyre!("Set BULK_PRIVATE_KEY env var"))?;

    let mut mm = RandomMarketMaker::initialize(config, &private_key).await?;

    // Graceful shutdown on Ctrl-C / SIGTERM
    let stop = mm.stop_handle();
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        info!("Signal received, stopping …");
        let _ = stop.send(false);
    });

    mm.run().await
}