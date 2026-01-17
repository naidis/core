use std::net::SocketAddr;

use clap::Parser;
use jsonrpsee::server::Server;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod rpc;

#[derive(Parser, Debug)]
#[command(name = "naidis-core")]
#[command(about = "Naidis Core Engine - JSON-RPC server for AI/PDF/YouTube/RSS processing")]
struct Args {
    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,

    #[arg(short, long, default_value = "9123")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("naidis_core=info".parse()?))
        .init();

    let args = Args::parse();
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    let server = Server::builder().build(addr).await?;
    let handle = server.start(rpc::create_router());

    info!("Naidis Core server running on {}", addr);

    handle.stopped().await;
    Ok(())
}
