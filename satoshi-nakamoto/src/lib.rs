//! Future client implementation for nakamoto

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::net::TcpStream;

use nakamoto_client::handle::Handle;
use nakamoto_client::model::Tip;
pub use nakamoto_client::Config;
pub use nakamoto_client::{Client, Error, Event, Network};
use nakamoto_common::bitcoin::consensus::{deserialize, serialize};
use nakamoto_common::bitcoin::Txid;
use nakamoto_common::bitcoin_hashes::hex::{FromHex, ToHex};
use nakamoto_common::block::{BlockHash, Height, Transaction};
use nakamoto_net_poll::{Reactor, Waker};
use nakamoto_p2p::fsm::fees::FeeEstimate;
use serde_json::{json, Value};

use clightningrpc_common::json_utils;
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;

use satoshi_common::client::SatoshiBackend;
use satoshi_common::utils::{bitcoin_hashes, hex};

#[derive(Clone)]
pub struct Nakamoto {
    network: Network,
    handler: nakamoto_client::Handle<Waker>,
    current_height: Cell<Option<Height>>,
}

impl Nakamoto {
    pub fn new(config: Config) -> Result<Self, Error> {
        let nakamoto = Client::<Reactor<TcpStream>>::new()?;
        let handler = nakamoto.handle();
        let network = config.network;
        // FIXME: join this later
        let _worker = std::thread::spawn(|| nakamoto.run(config));
        let client = Nakamoto {
            handler,
            network,
            current_height: Cell::new(None),
        };

        Ok(client)
    }

    fn build_estimate_fees(&self, fees: FeeEstimate) -> Result<Value, PluginError> {
        let mut resp = json_utils::init_payload();
        let medium: i64 = fees.median.try_into().unwrap();
        let low: i64 = fees.low.try_into().unwrap();
        json_utils::add_number(&mut resp, "opening", medium);
        json_utils::add_number(&mut resp, "mutual_close", low);
        json_utils::add_number(&mut resp, "unilateral_close", low);
        json_utils::add_number(&mut resp, "delayed_to_us", low);
        json_utils::add_number(&mut resp, "htlc_resolution", low);
        json_utils::add_number(&mut resp, "penalty", low);
        json_utils::add_number(&mut resp, "min_acceptable", low);
        json_utils::add_number(&mut resp, "max_acceptable", medium);
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

    // FIXME: return the correct error
    pub fn stop(self) -> Result<(), Error> {
        self.handler.shutdown()?;
        Ok(())
    }
}

fn from<T: Display>(err: T) -> PluginError {
    error!("{err}")
}

impl<T: Clone> SatoshiBackend<T> for Nakamoto {
    fn sync_block_by_height(&self, _p: &mut Plugin<T>, height: u64) -> Result<Value, PluginError> {
        let mut response = json_utils::init_payload();
        let header = self.handler.get_block_by_height(height).unwrap();
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

    fn sync_chain_info(&self, _: &mut Plugin<T>) -> Result<Value, PluginError> {
        match self.handler.get_tip() {
            Ok(Tip { height, .. }) => {
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

                // FIXME: need to be supported
                json_utils::add_bool(&mut resp, "ibd", false);
                Ok(resp)
            }
            Err(err) => Err(error!("{err}")),
        }
    }

    fn sync_estimate_fees(&self, _: &mut Plugin<T>) -> Result<Value, PluginError> {
        let Some(height) = self.current_height.get() else {
            return self.null_estimate_fees();
        };
        let Some(fees) = self.handler.estimate_feerate(height - 6).map_err(from)? else {
            return self.null_estimate_fees();
        };
        self.build_estimate_fees(fees)
    }

    fn sync_get_utxo(&self, _: &mut Plugin<T>, txid: &str, idx: u64) -> Result<Value, PluginError> {
        let txid = Txid::from_hex(txid).unwrap();
        let Some(utxo) = self.handler.get_utxo(&txid, idx.try_into().unwrap()).map_err(from)? else {
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
