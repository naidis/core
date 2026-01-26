// Allow dead code for WIP API functions not yet exposed via RPC
#![allow(dead_code)]

mod ai;
mod audio;
mod dataview;
mod epub;
mod git;
mod highlights;
mod integrations;
mod labels;
mod newsletter;
mod nlp;
mod pdf;
mod periodic;
mod reading;
mod rpc;
mod rss;
mod spaced_repetition;
mod tables;
mod tasks;
mod tier;
mod tts;
mod utils;
mod web_clip;
mod youtube;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "naidis-core")]
#[command(about = "Naidis Core - Backend engine for PKM workstation")]
struct Cli {
    #[arg(long, default_value = "http")]
    mode: String,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value = "21420")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.mode.as_str() {
        "http" => {
            tracing::info!(
                "Starting naidis-core HTTP server on {}:{}",
                cli.host,
                cli.port
            );
            rpc::run_http_server(&cli.host, cli.port).await
        }
        "stdio" => {
            tracing::info!("Starting naidis-core JSON-RPC server (stdio mode)");
            rpc::run_stdio_server().await
        }
        _ => {
            anyhow::bail!("Invalid mode: {}. Use 'http' or 'stdio'", cli.mode)
        }
    }
}
