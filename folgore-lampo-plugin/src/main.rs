//! Folgore as a lampo plugin.
//!
//! The gRPC server is [`lampo_plugin_sdk`]. Lampo spawns this binary with
//! `--lampo-listen` when `--help` mentions that flag. Chain methods come
//! from electrum unless `--mempool-space-url` is set, the same switch
//! phoenixd makes.
//!
//! ```sh
//! lampod-cli --plugin ./target/release/folgore-lampo
//! ```

mod backend;
mod recovery;

#[cfg(test)]
use std::sync::Once;

#[cfg(test)]
static INIT: Once = Once::new();

#[cfg(test)]
fn configure_tests() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

use std::sync::Arc;

use lampo_plugin_sdk::{FailureMode, OptionType, Plugin};
use serde_json::Value;
use tokio::sync::RwLock;

use backend::Backend;

#[tokio::main]
async fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
    // `--help` must mention `--lampo-listen` or lampod will not spawn us.
    // The SDK prints that and exits; chain flags are documented here too.
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        usage();
        std::process::exit(0);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let backend = Arc::new(RwLock::new(Backend::from_args(&args).unwrap_or_else(
        |err| {
            log::error!("{err}");
            std::process::exit(1);
        },
    )));

    let chain = backend.clone();
    Plugin::new()
        .failure_mode(FailureMode::FailClosed)
        .important(true)
        .option(
            "mempool-space-url",
            OptionType::String,
            None,
            "Esplora API to use instead of electrum",
        )
        .option(
            "electrum-server",
            OptionType::String,
            None,
            "Electrum server host:port",
        )
        .rpc_method(
            "getblockchaininfo",
            "Chain tip",
            "",
            rpc(chain.clone(), "getblockchaininfo"),
        )
        .rpc_method(
            "getblock",
            "Block by hash",
            "hash verbosity",
            rpc(chain.clone(), "getblock"),
        )
        .rpc_method(
            "getblockheader",
            "Header by hash",
            "hash",
            rpc(chain.clone(), "getblockheader"),
        )
        .rpc_method(
            "estimatesmartfee",
            "Fee estimate",
            "blocks",
            rpc(chain.clone(), "estimatesmartfee"),
        )
        .rpc_method(
            "getmempoolinfo",
            "Mempool min fee",
            "",
            rpc(chain.clone(), "getmempoolinfo"),
        )
        .rpc_method(
            "sendrawtransaction",
            "Broadcast a raw transaction",
            "hex",
            rpc(chain.clone(), "sendrawtransaction"),
        )
        .on_init(move |params| {
            let backend = backend.clone();
            async move {
                // CLI flags already chose the backend. A conf option overrides
                // that, matching `lampod-cli --plugin folgore-lampo -- --flag`.
                if let Some(next) = Backend::from_init(&params) {
                    log::info!("backend from init options");
                    *backend.write().await = next?;
                }
                Ok(())
            }
        })
        .start()
        .await;
}

fn usage() {
    eprintln!("Usage: folgore-lampo --lampo-listen 127.0.0.1:0");
    eprintln!("  --lampo-listen <addr>     loopback gRPC address");
    eprintln!("  --mempool-space-url <url> use this esplora API instead of electrum");
    eprintln!("  --electrum-server <host:port>");
    eprintln!("  --network <bitcoin|testnet|signet|regtest>");
    eprintln!("  --core-url <http://host:port>  bitcoind RPC, with --core-user and --core-pass");
}

fn rpc(
    backend: Arc<RwLock<Backend>>,
    method: &'static str,
) -> impl Fn(
    Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, String>> + Send>>
       + Send
       + Sync
       + 'static {
    move |params: Value| {
        let backend = backend.clone();
        let method = method.to_owned();
        Box::pin(async move {
            let guard = backend.read().await;
            guard.call(&method, &params)
        })
    }
}
