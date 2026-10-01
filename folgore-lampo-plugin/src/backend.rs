//! Which folgore backend answers lampo's chain calls.
//!
//! Electrum is the default, matching phoenixd. `--mempool-space-url` uses
//! [`folgore_esplora::Esplora`] instead. The HTTP and Electrum clients live
//! in those crates; this module only chooses one. The gRPC server is
//! `lampo-plugin-sdk`, not this crate.
use std::sync::Arc;

use folgore_bitcoind::BitcoinCore;
use folgore_common::client::FolgoreBackend;
use folgore_common::cln::plugin::errors::PluginError;
use folgore_electrum::Electrum;
use folgore_esplora::Esplora;

use crate::recovery::TimeoutRetry;

pub enum Backend {
    Electrum(Electrum),
    Esplora(Esplora<TimeoutRetry>),
    /// A real bitcoind. This is the only backend that returns cumulative
    /// chainwork, which lampo needs to sync past genesis.
    Bitcoin(BitcoinCore),
}

impl Backend {
    pub fn from_args(args: &[String]) -> Result<Self, String> {
        if let Some(url) = flag(args, "--core-url").filter(|url| !url.is_empty()) {
            let user = flag(args, "--core-user").unwrap_or_default();
            let pass = flag(args, "--core-pass").unwrap_or_default();
            log::info!("bitcoind {url}");
            let client = BitcoinCore::new(&url, &user, &pass).map_err(|err| err.to_string())?;
            return Ok(Self::Bitcoin(client));
        }
        if let Some(url) = url_from_args(args) {
            log::info!("mempool.space {url}");
            let client = Esplora::new("bitcoin", Some(url), Arc::new(TimeoutRetry::default()), "")
                .map_err(|err| err.to_string())?;
            return Ok(Self::Esplora(client));
        }
        let network = flag(args, "--network").unwrap_or_else(|| "bitcoin".into());
        let server = flag(args, "--electrum-server");
        let client = Electrum::new(&network, server).map_err(|err| err.to_string())?;
        Ok(Self::Electrum(client))
    }

    /// Conf options from lampo `init`. `None` means the CLI choice stands.
    ///
    /// `--core-url` on the command line already selected bitcoind. A
    /// mempool URL in init switches to esplora. A localhost URL here is
    /// lampo's own dummy core URL and is ignored, so a regtest tip cannot
    /// leak into a testnet sync.
    pub fn from_init(params: &serde_json::Value) -> Option<Result<Self, String>> {
        let url = params
            .pointer("/options/mempool-space-url")
            .and_then(|item| item.as_str())
            .filter(|url| !url.is_empty())?;
        if url.contains("127.0.0.1") || url.contains("localhost") {
            return None;
        }
        if !url.starts_with("http") {
            return None;
        }
        log::info!("mempool.space {url}");
        Some(
            Esplora::new(
                "bitcoin",
                Some(url.to_owned()),
                Arc::new(TimeoutRetry::default()),
                "",
            )
            .map(Self::Esplora)
            .map_err(|err| err.to_string()),
        )
    }

    pub fn call(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let args = match params {
            serde_json::Value::Array(items) => items.as_slice(),
            _ => &[],
        };
        let result = match self {
            Self::Electrum(client) => dispatch(client, method, args),
            Self::Esplora(client) => dispatch(client, method, args),
            Self::Bitcoin(client) => dispatch(client, method, args),
        };
        result.map_err(|err| err.to_string())
    }
}

fn dispatch(
    backend: &impl FolgoreBackend<()>,
    method: &str,
    args: &[serde_json::Value],
) -> folgore_common::Result<serde_json::Value> {
    match method {
        "getblockchaininfo" => backend.chain_tip(),
        "getblockheader" => backend.chain_header(arg_str(args, 0)),
        "getblock" => backend.chain_block(arg_str(args, 0), arg_u64(args, 1).unwrap_or(1)),
        "estimatesmartfee" => backend.chain_fee(arg_u64(args, 0).unwrap_or(6)),
        "getmempoolinfo" => backend.chain_mempool(),
        "sendrawtransaction" => backend.chain_broadcast(arg_str(args, 0)),
        // ldk-node's transaction sync. These do not need cumulative chainwork.
        "esplora_tip" => esplora_tip(backend),
        "esplora_header" => esplora_header_by_hash(backend, arg_str(args, 0)),
        "esplora_tx_status" => esplora_tx_status(backend, arg_str(args, 0)),
        "esplora_merkle" => esplora_merkle(backend, arg_str(args, 0)),
        "esplora_output" => {
            esplora_output(backend, arg_str(args, 0), arg_u64(args, 1).unwrap_or(0))
        }
        "esplora_tx" => esplora_tx(backend, arg_str(args, 0)),
        other => Err(folgore_common::cln::plugin::error!(
            "method not found: {other}"
        )),
    }
}

fn esplora_tip(backend: &impl FolgoreBackend<()>) -> folgore_common::Result<serde_json::Value> {
    match backend.kind() {
        folgore_common::client::BackendKind::Esplora => backend.chain_tip(),
        folgore_common::client::BackendKind::Electrum => backend.chain_tip(),
        _ => Err(folgore_common::cln::plugin::error!(
            "this backend has no transaction-sync tip"
        )),
    }
}

fn esplora_header_by_hash(
    backend: &impl FolgoreBackend<()>,
    hash: &str,
) -> folgore_common::Result<serde_json::Value> {
    backend.chain_header(hash)
}

fn esplora_tx_status(
    backend: &impl FolgoreBackend<()>,
    txid: &str,
) -> folgore_common::Result<serde_json::Value> {
    backend.chain_tx_status(txid)
}

fn esplora_merkle(
    backend: &impl FolgoreBackend<()>,
    txid: &str,
) -> folgore_common::Result<serde_json::Value> {
    backend.chain_tx_merkle(txid)
}

fn esplora_tx(
    backend: &impl FolgoreBackend<()>,
    txid: &str,
) -> folgore_common::Result<serde_json::Value> {
    backend.chain_tx(txid)
}

fn esplora_output(
    backend: &impl FolgoreBackend<()>,
    txid: &str,
    vout: u64,
) -> folgore_common::Result<serde_json::Value> {
    backend.chain_output_status(txid, vout)
}

fn arg_str(args: &[serde_json::Value], index: usize) -> &str {
    args.get(index)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

fn arg_u64(args: &[serde_json::Value], index: usize) -> Option<u64> {
    args.get(index).and_then(|value| value.as_u64())
}

pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1).cloned())
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix(&format!("{name}=")).map(str::to_owned))
        })
}

fn url_from_args(args: &[String]) -> Option<String> {
    flag(args, "--mempool-space-url").filter(|url| !url.is_empty())
}
