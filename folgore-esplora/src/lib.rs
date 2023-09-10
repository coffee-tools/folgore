#![deny(clippy::unwrap_used)]
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

use log::{debug, info};
use serde_json::json;

use clightningrpc_common::json_utils;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use esplora_client::api::{FromHex, Transaction, TxOut, Txid};
use esplora_client::{deserialize, serialize, BlockSummary};
use esplora_client::{BlockingClient, Builder};

use folgore_common::client::FolgoreBackend;
use folgore_common::cln_plugin::types::LogLevel;
use folgore_common::stragegy::RecoveryStrategy;
use folgore_common::utils::ByteBuf;
use folgore_common::utils::{bitcoin_hashes, hex};

#[derive(Clone)]
enum Network {
    Bitcoin(String),
    Testnet(String),
    #[allow(dead_code)]
    Liquid(String),
    BitcoinTor(String),
    TestnetTor(String),
    #[allow(dead_code)]
    LiquidTor(String),
}

impl Network {
    pub fn url(&self) -> String {
        match &self {
            Self::Bitcoin(url) => url.to_string(),
            Self::Liquid(url) => url.to_string(),
            Self::Testnet(url) => url.to_string(),
            Self::BitcoinTor(url) => url.to_string(),
            Self::TestnetTor(url) => url.to_string(),
            Self::LiquidTor(url) => url.to_string(),
        }
    }
}

impl TryFrom<&str> for Network {
    type Error = PluginError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "bitcoin" => Ok(Self::Bitcoin("https://mempool.space/api/v1".to_owned())),
            "bitcoin/tor" => Ok(Self::BitcoinTor(
                "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion/api"
                    .to_owned(),
            )),
            "testnet" => Ok(Self::Testnet(
                "https://mempool.space/testnet/api/v1".to_owned(),
            )),
            "testnet/tor" => Ok(Self::TestnetTor(
                "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion/testnet/api"
                    .to_owned(),
            )),
            "signet" => Ok(Self::Testnet(
                "https://mempool.space/signet/api/v1".to_owned(),
            )),
            _ => Err(error!("network {value} not supported")),
        }
    }
}

// FIXME: move this inside the Plugin API to map the error
/// convert the error to a plugin error
fn from<T: Display>(value: T) -> PluginError {
    PluginError::new(-1, &format!("{value}"), None)
}

#[derive(Clone)]
pub struct Esplora<R: RecoveryStrategy> {
    client: BlockingClient,
    recovery_strategy: Arc<R>,
}

impl<R: RecoveryStrategy> Esplora<R> {
    pub fn new(network: &str, url: Option<String>, strategy: Arc<R>) -> Result<Self, PluginError> {
        let url = if let Some(url) = url {
            url
        } else {
            let network = Network::try_from(network)?;
            network.url()
        };
        let builder = Builder::new(&url);
        Ok(Self {
            client: builder.build_blocking().map_err(|err| error!("{err}"))?,
            recovery_strategy: strategy,
        })
    }
}

fn fee_in_range(estimation: &HashMap<String, f64>, from: u64, to: u64) -> Option<i64> {
    for rate in from..to {
        let key = &format!("{rate}");
        if estimation.contains_key(key) {
            return Some(estimation[key] as i64);
        }
    }
    None
}

impl<T: Clone, S: RecoveryStrategy> FolgoreBackend<T> for Esplora<S> {
    fn kind(&self) -> folgore_common::client::BackendKind {
        folgore_common::client::BackendKind::Esplora
    }

    fn sync_block_by_height(
        &self,
        plugin: &mut clightningrpc_plugin::plugin::Plugin<T>,
        height: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let fail_resp = json!({
            "blockhash": null,
            "block": null,
        });

        let chain_tip = self.client.get_height().map_err(|err| error!("{err}"))?;
        if height > chain_tip.into() {
            let resp = json!({
                "blockhash": null,
                "block": null,
            });
            return Ok(resp);
        }
        // Check if the blocks that core lightning wants exist.
        let current_height = self
            .recovery_strategy
            .apply(|| self.client.get_height().map_err(|err| error!("{err}")))
            .map_err(|err| error!("{err}"))?;
        if current_height < height as u32 {
            plugin.log(
                LogLevel::Debug,
                &format!("requesting block out of best chain. Block height wanted: {height}"),
            );
            return Ok(fail_resp);
        }

        // Now that we are sure that the block exist we can requesting it
        let block: Vec<BlockSummary> = self.recovery_strategy.apply(|| {
            self.client
                .get_blocks(Some(height.try_into().expect("height convertion fails")))
                .map_err(|err| error!("{err}"))
        })?;
        let block_hash = block.first().ok_or(error!("block not found"))?;
        let block_hash = block_hash.id;

        let block = self
            .recovery_strategy
            .apply(|| self.client.get_block_by_hash(&block_hash).map_err(from))?;

        let mut response = json_utils::init_payload();
        if let Some(block) = block {
            json_utils::add_str(&mut response, "blockhash", &block_hash.to_string());
            let ser = serialize(&block);
            let bytes = ByteBuf(ser.as_slice());
            json_utils::add_str(&mut response, "block", &format!("{:02x}", bytes));
            return Ok(response);
        }
        debug!("block not found!");
        Ok(fail_resp)
    }

    fn sync_chain_info(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        _: Option<u64>,
    ) -> Result<serde_json::Value, PluginError> {
        let current_height = self.client.get_height().map_err(from)?;
        info!("blockchain height: {current_height}");
        let genesis = self
            .recovery_strategy
            .apply(|| self.client.get_blocks(Some(0)).map_err(from))?;

        let genesis = genesis.first().ok_or(error!("genesis block not found"))?;
        let network = match genesis.id.to_string().as_str() {
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f" => "main",
            "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943" => "test",
            "1466275836220db2944ca059a3a10ef6fd2ea684b0688d2c379296888a206003" => "liquidv1",
            _ => return Err(error!("wrong chain hash {}", genesis.id)),
        };

        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "chain", network);
        json_utils::add_number(&mut response, "headercount", current_height.into());
        json_utils::add_number(&mut response, "blockcount", current_height.into());
        json_utils::add_bool(&mut response, "ibd", false);
        Ok(response)
    }

    fn sync_estimate_fees(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, PluginError> {
        let fee_rates = self
            .recovery_strategy
            .apply(|| self.client.get_fee_estimates().map_err(from))?;

        // FIXME: if some of the valus is none, we should return
        // a empity response to cln, see the satoshi backend docs
        let hight =
            fee_in_range(&fee_rates, 2, 10).ok_or(error!("fee in the range [2, 10] not found"))?;
        let urgent =
            fee_in_range(&fee_rates, 6, 15).ok_or(error!("fee in the range [6, 15] not found"))?;
        let normal = fee_in_range(&fee_rates, 12, 24)
            .ok_or(error!("fee in the range [12, 24] not found"))?;
        let slow = fee_in_range(&fee_rates, 60, 170)
            .ok_or(error!("fee in the range [60, 17] not found"))?;

        // FIXME: manage to return an empty response when there is some error
        let mut resp = json_utils::init_payload();
        json_utils::add_number(&mut resp, "opening", normal);
        json_utils::add_number(&mut resp, "mutual_close", slow);
        json_utils::add_number(&mut resp, "unilateral_close", urgent);
        json_utils::add_number(&mut resp, "delayed_to_us", normal);
        json_utils::add_number(&mut resp, "htlc_resolution", urgent);
        json_utils::add_number(&mut resp, "penalty", normal);
        json_utils::add_number(&mut resp, "min_acceptable", slow / 2);
        json_utils::add_number(&mut resp, "max_acceptable", hight * 10);
        Ok(resp)
    }

    fn sync_get_utxo(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        txid: &str,
        idx: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let txid = Txid::from_hex(txid).map_err(from)?;
        let utxo = self
            .recovery_strategy
            .apply(|| self.client.get_tx(&txid).map_err(from))?;

        let mut resp = json_utils::init_payload();
        if let Some(tx) = utxo {
            let output: TxOut = tx.output[idx as usize].clone();
            json_utils::add_number(&mut resp, "amount", output.value.try_into().map_err(from)?);
            json_utils::add_str(&mut resp, "script", &format!("{:x}", output.script_pubkey));
            return Ok(resp);
        }
        Ok(json!({
            "amount": null,
            "script": null,
        }))
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        tx: &str,
        _with_hight_fee: bool,
    ) -> Result<serde_json::Value, PluginError> {
        let tx = hex!(tx);
        let tx: Result<Transaction, _> = deserialize(&tx);
        debug!("the transaction deserialised is {:?}", tx);
        let tx = tx.map_err(from)?;
        let tx_send = self
            .recovery_strategy
            .apply(|| Ok(self.client.broadcast(&tx)))?;

        let mut resp = json_utils::init_payload();
        json_utils::add_bool(&mut resp, "success", tx_send.is_ok());
        if let Err(err) = tx_send {
            json_utils::add_str(&mut resp, "errmsg", &err.to_string());
        }
        Ok(resp)
    }
}
