//! Folgore bitcoin core implementation
//!
//! This is an implemetation of the folgore
//! backend with the bitcoin core node.
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
#![deny(clippy::unwrap_used)]
use std::collections::HashMap;
use std::str::FromStr;

use bitcoincore_rpc::bitcoin::consensus::{deserialize, serialize};
use bitcoincore_rpc::bitcoin::Transaction;
use bitcoincore_rpc::bitcoin::Txid;
use bitcoincore_rpc::bitcoincore_rpc_json::EstimateMode;
use bitcoincore_rpc::jsonrpc::serde_json::json;
use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::{Auth, Client};

use folgore_common::client::fee_estimator::FeeEstimator;
use folgore_common::client::fee_estimator::{FeePriority, FEE_RATES};
use folgore_common::client::FolgoreBackend;
use folgore_common::cln::plugin::types::LogLevel;
use folgore_common::hex;
use folgore_common::prelude::cln_plugin::error;
use folgore_common::prelude::cln_plugin::errors;
use folgore_common::prelude::cln_plugin::errors::PluginError;
use folgore_common::prelude::cln_plugin::plugin;
use folgore_common::prelude::json;
use folgore_common::utils::ByteBuf;

pub struct BitcoinCore {
    pub client: Client,
}

impl BitcoinCore {
    pub fn new(url: &str, user: &str, pass: &str) -> Result<Self, errors::PluginError> {
        let client = Client::new(url, Auth::UserPass(user.to_owned(), pass.to_owned()))
            .map_err(|err| error!("{err}"))?;
        Ok(Self { client })
    }
}

impl<T: Clone> FolgoreBackend<T> for BitcoinCore {
    fn kind(&self) -> folgore_common::client::BackendKind {
        folgore_common::client::BackendKind::BitcoinCore
    }

    fn sync_chain_info(
        &self,
        _: &mut plugin::Plugin<T>,
        _: Option<u64>,
    ) -> Result<json::Value, errors::PluginError> {
        let chaininfo = self
            .client
            .get_blockchain_info()
            .map_err(|err| error!("{err}"))?;

        Ok(json::json!({
            "headercount": chaininfo.headers,
            "blockcount": chaininfo.blocks,
            "ibd": chaininfo.initial_block_download,
            "chain": chaininfo.chain,
        }))
    }

    fn sync_block_by_height(
        &self,
        plugin: &mut plugin::Plugin<T>,
        height: u64,
    ) -> Result<json::Value, errors::PluginError> {
        let current_height = self
            .client
            .get_block_count()
            .map_err(|err| error!("{err}"))?;
        if current_height < height {
            plugin.log(
                LogLevel::Debug,
                &format!("requesting block out of best chain. Block height wanted: {height}"),
            );
            return Ok(json!({
                "blockhash": null,
                "block": null,
            }));
        }
        let block_header = self
            .client
            .get_block_hash(height)
            .map_err(|err| error!("{err}"))?;
        let block = self
            .client
            .get_block(&block_header)
            .map_err(|err| error!("{err}"))?;

        let serialize = serialize(&block);
        let ser_str = serialize.as_slice();
        Ok(json::json!({
            "blockhash": block_header.to_string(),
            "block": json::to_value(format!("{:20x}", ByteBuf(ser_str)))?,
        }))
    }

    fn sync_estimate_fees(
        &self,
        _: &mut plugin::Plugin<T>,
    ) -> Result<json::Value, errors::PluginError> {
        let mut fee_map = HashMap::new();
        for FeePriority(block, target) in FEE_RATES.iter().cloned() {
            let diff = block as u64;
            let mode = match target {
                "CONSERVATIVE" => EstimateMode::Conservative,
                _ => {
                    return Err(error!(
                        "mode {target} unsupported by the plugin please report the bug"
                    ))
                }
            };
            let Ok(fees) = self.client.estimate_smart_fee(block, Some(mode)) else {
                continue;
            };
            let Some(fee) = fees.fee_rate else {
                continue;
            };
            fee_map.insert(diff, fee.to_sat());
        }
        if fee_map.len() != FEE_RATES.len() {
            return FeeEstimator::null_estimate_fees();
        }
        FeeEstimator::build_estimate_fees(&fee_map)
    }

    fn sync_get_utxo(
        &self,
        _: &mut plugin::Plugin<T>,
        txid: &str,
        idx: u64,
    ) -> Result<json::Value, errors::PluginError> {
        let utxo = self
            .client
            .get_tx_out(
                &Txid::from_str(txid).map_err(|err| error!("{err}"))?,
                idx as u32,
                None,
            )
            .map_err(|err| error!("{err}"))?;
        if utxo.is_none() {
            return Ok(json::json!({
                "script": null,
                "amount": null,
            }));
        }
        // SAFETY: this is safe to unwrap because we check the wrong case before
        // so this will never fails.
        #[allow(clippy::unwrap_used)]
        let utxo = utxo.unwrap();
        Ok(json::json!({
            "script": String::from_utf8(utxo.script_pub_key.hex).map_err(|err| error!("{err}"))?,
            "amount": utxo.value.to_sat(),
        }))
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut plugin::Plugin<T>,
        raw_tx: &str,
        _: bool,
    ) -> Result<json::Value, errors::PluginError> {
        use folgore_common::utils::bitcoin_hashes;
        let hex_tx = hex!(raw_tx);
        let tx: Transaction = deserialize(&hex_tx).map_err(|err| error!("{err}"))?;
        let result = self.client.send_raw_transaction(&tx);
        Ok(json::json!({
           "success": result.is_ok(),
            "errmsg": result.err().map(|err| err.to_string()),
        }))
    }
}
