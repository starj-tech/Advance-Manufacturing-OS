//! Mock OPC-UA server for development & CI.
//!
//! In PR #3 this becomes a real `opcua` server exposing synthetic
//! NodeIds (temperature sine wave + pressure noise) for end-to-end
//! testing of the OPC-UA bridge. For PR #1 we ship a placeholder that
//! prints a banner and exits, so downstream tests can compile against
//! the binary path even before the protocol is wired.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "opcua-mock-server")]
#[command(about = "Synthetic OPC-UA server for AETHER-OS development", version)]
struct Args {
    /// Endpoint to bind, e.g. opc.tcp://0.0.0.0:4840
    #[arg(long, default_value = "opc.tcp://127.0.0.1:4840")]
    endpoint: String,

    /// Number of synthetic tags to publish.
    #[arg(long, default_value_t = 4)]
    tags: u32,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    tracing::info!(endpoint = %args.endpoint, tags = args.tags,
        "[opcua-mock-server] PR #3 will implement the synthetic server here.");
    tracing::info!("Skeleton acknowledges launch; exiting cleanly.");
}
