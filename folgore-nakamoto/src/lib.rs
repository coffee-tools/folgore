//! Backland client implementation for nakamoto
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::Display;
use std::net::TcpStream;
use std::thread::JoinHandle;

use clightningrpc_plugin::types::LogLevel;
use nakamoto_client::FeeRate;
use serde_json::{json, Value};

use clightningrpc_common::json_utils;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use nakamoto_client::handle::Handle;
use nakamoto_client::model::Tip;
pub use nakamoto_client::Config;
pub use nakamoto_client::{Client, Error, Event, Network};
use nakamoto_common::bitcoin::consensus::{deserialize, serialize};
use nakamoto_common::bitcoin::Txid;
use nakamoto_common::bitcoin_hashes::hex::{FromHex, ToHex};
use nakamoto_common::block::{Height, Transaction};
use nakamoto_net_poll::{Reactor, Waker};

use folgore_common::client::FolgoreBackend;
use folgore_common::utils::{bitcoin_hashes, hex};

#[derive(Clone)]
struct FeePriority(u16, &'static str);

static FEE_RATES: [FeePriority; 4] = [
    FeePriority(2, "CONSERVATIVE"),
    FeePriority(6, "CONSERVATIVE"),
    FeePriority(12, "CONSERVATIVE"),
    FeePriority(100, "CONSERVATIVE"),
];

pub struct Nakamoto {
    network: Network,
    handler: nakamoto_client::Handle<Waker>,
    current_height: Cell<Option<Height>>,
    worker: Option<JoinHandle<Result<(), Error>>>,
}

impl Nakamoto {
    pub fn new(config: Config) -> Result<Self, Error> {
        let nakamoto = Client::<Reactor<TcpStream>>::new()?;
        let handler = nakamoto.handle();
        let network = config.network;
        let worker = std::thread::spawn(|| nakamoto.run(config));
        let client = Nakamoto {
            handler,
            network,
            current_height: Cell::new(None),
            worker: Some(worker),
        };

        Ok(client)
    }

    fn urgent_fee(&self, fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&6).copied()
    }

    fn hightest_fee(&self, fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&2).copied()
    }

    fn normal_fee(&self, fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&12).copied()
    }

    fn slow_fee(&self, fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&100).copied()
    }

    fn build_estimate_fees(&self, fees: &HashMap<u64, FeeRate>) -> Result<Value, PluginError> {
        let mut resp = json_utils::init_payload();
        let Some(high) = self.hightest_fee(fees) else {
            return Err(error!("highest fee not found"));
        };
        let Some(urgent) = self.urgent_fee(fees) else {
            return Err(error!("urgent fee not found"));
        };
        let Some(normal) = self.normal_fee(fees) else {
            return Err(error!("normal fee not found"));
        };
        let Some(slow) = self.slow_fee(fees) else {
            return Err(error!("slow fee not found"));
        };
        json_utils::add_number(&mut resp, "opening", normal as i64);
        json_utils::add_number(&mut resp, "mutual_close", slow as i64);
        json_utils::add_number(&mut resp, "unilateral_close", urgent as i64);
        json_utils::add_number(&mut resp, "delayed_to_us", normal as i64);
        json_utils::add_number(&mut resp, "htlc_resolution", urgent as i64);
        json_utils::add_number(&mut resp, "penalty", normal as i64);
        json_utils::add_number(&mut resp, "min_acceptable", (slow / 2) as i64);
        json_utils::add_number(&mut resp, "max_acceptable", (high * 2) as i64);
        Ok(resp)
    }

    fn null_estimate_fees(&self) -> Result<Value, PluginError> {
        Ok(json!({
            "opening": null,
            "mutual_close": null,
            "unilateral_close": null,
            "delayed_to_us": null,
            "htlc_resolution": null,
            "penalty": null,
            "min_acceptable": null,
            "max_acceptable": null,
        }))
    }
}

impl Drop for Nakamoto {
    fn drop(&mut self) {
        let _ = self.handler.clone().shutdown();
        let Some(worker) = self.worker.take() else {
            return;
        };
        let _ = worker.join();
    }
}

fn from<T: Display>(err: T) -> PluginError {
    error!("{err}")
}

impl<T: Clone> FolgoreBackend<T> for Nakamoto {
    fn sync_block_by_height(&self, _p: &mut Plugin<T>, height: u64) -> Result<Value, PluginError> {
        let mut response = json_utils::init_payload();
        let header = self.handler.get_block_by_height(height).map_err(from)?;
        let blk_chan = self.handler.blocks();
        if let None = header {
            return Ok(json!({
                "blockhash": null,
                "block": null,
            }));
        }

        let header = header.unwrap();
        if let Err(err) = self.handler.request_block(&header.block_hash()) {
            return Err(error!("{err}"));
        }

        self.current_height.set(Some(height.into()));
        json_utils::add_str(
            &mut response,
            "blockhash",
            header.block_hash().to_string().as_str(),
        );

        let (blk, _) = blk_chan.recv().unwrap();
        let serialize = serialize(&blk);
        let ser_str = serialize.as_slice().to_hex();
        json_utils::add_str(&mut response, "block", &ser_str);
        Ok(response)
    }

    fn sync_chain_info(
        &self,
        plugin: &mut Plugin<T>,
        known_height: Option<u64>,
    ) -> Result<Value, PluginError> {
        match self.handler.get_tip() {
            Ok(Tip { mut height, .. }) => {
                let mut is_sync = true;
                if Some(height) <= known_height {
                    while let Err(err) = self.handler.wait_for_height(known_height.unwrap()) {
                        plugin.log(LogLevel::Info, &format!("Waiting for block {height}...."));
                        plugin.log(
                            LogLevel::Debug,
                            &format!("while waiting the block we get an error {err}"),
                        )
                    }
                    height = known_height.unwrap();
                } else {
                    is_sync = false;
                }
                let mut resp = json_utils::init_payload();
                let height: i64 = height.try_into().unwrap();
                json_utils::add_number(&mut resp, "headercount", height);
                json_utils::add_number(&mut resp, "blockcount", height);
                let network = match self.network {
                    Network::Mainnet => "main",
                    Network::Testnet => "test",
                    Network::Regtest => "regtest",
                    Network::Signet => "signet",
                };
                json_utils::add_str(&mut resp, "chain", network);
                json_utils::add_bool(&mut resp, "ibd", is_sync);
                Ok(resp)
            }
            Err(err) => Err(error!("{err}")),
        }
    }

    fn sync_estimate_fees(&self, plugin: &mut Plugin<T>) -> Result<Value, PluginError> {
        let Some(height) = self.current_height.get() else {
            return self.null_estimate_fees();
        };
        let mut fee_map = HashMap::new();
        for FeePriority(block, _) in FEE_RATES.to_vec() {
            let diff = block as u64;
            let Ok(Some(fees)) = self.handler.estimate_feerate(height - diff) else {
                continue;
            };
            // We try to stay in a good range, so we take the high fee for the block
            // estimated by nakamto
            //
            // FIXME: is this a good assumtion?
            fee_map.insert(diff, fees.high);
        }
        plugin.log(
            LogLevel::Debug,
            &format!("fee estimated from nakamoto {:?}", fee_map),
        );
        if fee_map.len() != FEE_RATES.len() {
            return self.null_estimate_fees();
        }
        self.build_estimate_fees(&fee_map)
    }

    fn sync_get_utxo(&self, _: &mut Plugin<T>, txid: &str, idx: u64) -> Result<Value, PluginError> {
        let txid = Txid::from_hex(txid).unwrap();
        let Some(utxo) = self
            .handler
            .get_utxo(&txid, idx.try_into().unwrap())
            .map_err(from)?
        else {
            return Ok(json!({
                "amount": null,
                "script": null,
            }));
        };
        let mut resp = json_utils::init_payload();
        json_utils::add_number(&mut resp, "amount", utxo.value.try_into().unwrap());
        json_utils::add_str(&mut resp, "script", utxo.script_pubkey.to_hex().as_str());
        Ok(resp)
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut Plugin<T>,
        tx: &str,
        _: bool,
    ) -> Result<Value, PluginError> {
        let tx = hex!(tx);
        let tx: Transaction = deserialize(&tx).unwrap();
        let mut resp = json_utils::init_payload();
        if let Err(err) = self.handler.submit_transaction(tx) {
            json_utils::add_bool(&mut resp, "success", false);
            json_utils::add_str(&mut resp, "errmsg", &format!("{err}"));
        } else {
            json_utils::add_bool(&mut resp, "success", true);
        }
        Ok(resp)
    }
}
