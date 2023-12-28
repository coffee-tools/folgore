//! Backland client implementation for nakamoto
#![deny(clippy::unwrap_used)]
use std::cell::Cell;
use std::fmt::Display;
use std::net::TcpStream;
use std::sync::Mutex;
use std::thread::JoinHandle;

use folgore_common::stragegy::RecoveryStrategy;
use folgore_esplora::Esplora;
use nakamoto_client::handle::Handle;
use nakamoto_common::bitcoin::consensus::{deserialize, serialize};
use nakamoto_common::bitcoin::Txid;
use nakamoto_common::bitcoin_hashes::hex::{FromHex, ToHex};
use nakamoto_common::block::{Height, Transaction};
use nakamoto_net_poll::{Reactor, Waker};
use serde_json::{json, Value};

use folgore_common::client::FolgoreBackend;
use folgore_common::cln::json_utils;
use folgore_common::cln::plugin::error;
use folgore_common::cln::plugin::errors::PluginError;
use folgore_common::cln::plugin::plugin::Plugin;
use folgore_common::cln::plugin::types::LogLevel;
use folgore_common::utils::{bitcoin_hashes, hex};

pub use nakamoto_client::Config;
pub use nakamoto_client::{Client, Error, Event, Network};

pub struct Nakamoto<R: RecoveryStrategy> {
    network: Network,
    handler: nakamoto_client::Handle<Waker>,
    current_height: Mutex<Cell<Option<Height>>>,
    worker: Option<JoinHandle<Result<(), Error>>>,
    esplora: Esplora<R>,
}

impl<R: RecoveryStrategy> Nakamoto<R> {
    pub fn new(config: Config, esplora: Esplora<R>) -> Result<Self, Error> {
        let nakamoto = Client::<Reactor<TcpStream>>::new()?;
        let handler = nakamoto.handle();
        let network = config.network;
        let worker = std::thread::spawn(|| nakamoto.run(config));
        let client = Nakamoto {
            handler,
            network,
            esplora,
            current_height: Mutex::new(Cell::new(None)),
            worker: Some(worker),
        };

        Ok(client)
    }
}

impl<R: RecoveryStrategy> Drop for Nakamoto<R> {
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

impl<T: Clone, R: RecoveryStrategy> FolgoreBackend<T> for Nakamoto<R> {
    fn kind(&self) -> folgore_common::client::BackendKind {
        folgore_common::client::BackendKind::Nakamoto
    }

    fn sync_block_by_height(&self, _p: &mut Plugin<T>, height: u64) -> Result<Value, PluginError> {
        let mut response = json_utils::init_payload();
        let header = self.handler.get_block_by_height(height).map_err(from)?;
        let blk_chan = self.handler.blocks();
        if header.is_none() {
            return Ok(json!({
                "blockhash": null,
                "block": null,
            }));
        }

        let header = header.ok_or(error!("header not found inside the block"))?;
        if let Err(err) = self.handler.request_block(&header.block_hash()) {
            return Err(error!("{err}"));
        }

        self.current_height
            .lock()
            .map_err(|err| error!("{err}"))?
            .set(Some(height));
        json_utils::add_str(
            &mut response,
            "blockhash",
            header.block_hash().to_string().as_str(),
        );

        let (blk, _) = blk_chan.recv().map_err(|err| error!("{err}"))?;
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
            Ok((mut height, ..)) => {
                let mut is_sync = true;
                if Some(height) <= known_height {
                    while let Err(err) = self
                        .handler
                        .wait_for_height(known_height.ok_or(error!("known height not found"))?)
                    {
                        plugin.log(LogLevel::Info, &format!("Waiting for block {height}...."));
                        plugin.log(
                            LogLevel::Debug,
                            &format!("while waiting the block we get an error {err}"),
                        )
                    }
                    height = known_height.ok_or(error!("known height not found"))?;
                } else {
                    is_sync = false;
                }
                let mut resp = json_utils::init_payload();
                let height: i64 = height.try_into().map_err(|err| error!("{err}"))?;
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

    // FIXME: we can use the neutrino API here that it is just a json
    fn sync_estimate_fees(&self, plugin: &mut Plugin<T>) -> Result<Value, PluginError> {
        self.esplora.sync_estimate_fees(plugin)
    }

    fn sync_get_utxo(&self, _: &mut Plugin<T>, txid: &str, idx: u64) -> Result<Value, PluginError> {
        let txid = Txid::from_hex(txid).map_err(|err| error!("{err}"))?;
        let Some(utxo) = self
            .handler
            .get_submitted_transaction(&txid)
            .map_err(from)?
        else {
            return Ok(json!({
                "amount": null,
                "script": null,
            }));
        };
        let utxo = &utxo.output[idx as usize];
        let mut resp = json_utils::init_payload();
        json_utils::add_number(
            &mut resp,
            "amount",
            utxo.value.try_into().map_err(|err| error!("{err}"))?,
        );
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
        let tx: Transaction = deserialize(&tx).map_err(|err| error!("{err}"))?;
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
