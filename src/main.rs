use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use tracing_subscriber::{fmt, EnvFilter};

mod server;
mod session;
mod settings;

#[derive(Debug, Parser)]
#[command(name = "air_traffic_control")] 
#[command(about = "Headless AI coding agent", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Start {
        #[arg(long, default_value = "127.0.0.1:7171")]
        listen: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start { listen } => {
            let addr: SocketAddr = listen.parse()?;
            let state = server::AppState::default();
            server::serve(addr, state).await?;
        }
    }
    Ok(())
}
