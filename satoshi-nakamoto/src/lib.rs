//! Future client implementation for nakamoto

use std::cell::Cell;
use std::collections::BTreeMap;
use std::net::TcpStream;

use clightningrpc_plugin::types::LogLevel;
use nakamoto_client::handle::Handle;
use nakamoto_client::model::Tip;
pub use nakamoto_client::Config;
pub use nakamoto_client::{Client, Error, Event, Network};
use nakamoto_common::bitcoin::consensus::{deserialize, serialize};
use nakamoto_common::bitcoin_hashes::hex::ToHex;
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
    fees: BTreeMap<Height, FeeEstimate>,
    last_hash: Cell<Option<BlockHash>>,
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
            fees: BTreeMap::new(),
            last_hash: Cell::new(None),
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

impl<T: Clone> SatoshiBackend<T> for Nakamoto {
    fn sync_block_by_height(
        &self,
        plugin: &mut Plugin<T>,
        height: u64,
    ) -> Result<Value, PluginError> {
        let mut response = json_utils::init_payload();
        let header = self.handler.get_block_by_height(height).unwrap();
        let blk_chan = self.handler.blocks();
        if let None = header {
            // FIXME: this need to be improved
            return Ok(response);
        }

        let header = header.unwrap();
        if let Err(err) = self.handler.request_block(&header.block_hash()) {
            let err = PluginError::new(1, err.to_string().as_str(), None);
            return Err(err);
        }

        self.last_hash.set(Some(header.block_hash()));
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
        let Some(last_hash) = self.last_hash.get() else {
            return self.null_estimate_fees();
        };
        let _ = self.handler.request_block(&last_hash);
        let Ok((_, Some(fees))) = self.handler.wait(|event| {
            if let nakamoto_p2p::Event::Inventory(nakamoto_p2p::fsm::InventoryEvent::BlockProcessed { height, fees, ..}) = event {
                Some((height, fees))
            } else {
                None
            }
        }) else {
            return self.null_estimate_fees();
        };
        self.build_estimate_fees(fees)
    }

    fn sync_get_utxo(&self, _: &mut Plugin<T>, _: &str, _: u64) -> Result<Value, PluginError> {
        todo!()
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
