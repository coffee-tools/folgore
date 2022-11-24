//! Future client implementation for nakamoto
use clightningrpc_common::json_utils;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use future_common::client::FutureBackend;
use nakamoto_client::chan::Receiver;
use nakamoto_client::handle::Handle;
use nakamoto_client::model::Tip;
use nakamoto_client::{Client, Config, Error, Network};
use nakamoto_common::bitcoin::consensus::{deserialize, serialize};
use nakamoto_common::block::{Block, Height, Transaction};
use nakamoto_net_poll::{Reactor, Waker};
use serde_json::Value;
use std::net::TcpStream;
use std::thread::JoinHandle;

struct Nakamoto {
    network: Network,
    handler: nakamoto_client::Handle<Waker>,
    worker: JoinHandle<Result<(), Error>>,
}

impl Nakamoto {
    pub fn new(config: Config) -> Result<Self, Error> {
        let nakamoto = Client::<Reactor<TcpStream>>::new()?;
        let handler = nakamoto.handle();
        let network = config.network;
        let worker = std::thread::spawn(|| nakamoto.run(config));
        let client = Nakamoto {
            handler,
            worker,
            network,
        };
        Ok(client)
    }

    // FIXME: return the correct error
    pub fn stop(self) -> Result<(), Error> {
        self.handler.shutdown()?;
        let _ = self.worker.join();
        Ok(())
    }
}

impl<T: Clone> FutureBackend<T> for Nakamoto {
    type Error = PluginError;

    fn sync_block_by_height(&self, _: &mut Plugin<T>, height: u64) -> Result<Value, Self::Error> {
        let mut response = json_utils::init_payload();
        let header = self.handler.get_block_by_height(height).unwrap();
        let blk_chan = self.handler.blocks();
        if let None = header {
            // FIXME: this need to be improved
            return Ok(response);
        }

        // FIXME: get the from the stream!

        let header = header.unwrap();
        if let Err(err) = self.handler.request_block(&header.block_hash()) {
            let err = PluginError::new(1, err.to_string().as_str(), None);
            return Err(err);
        }

        json_utils::add_str(
            &mut response,
            "blockhash",
            header.block_hash().to_string().as_str(),
        );

        let (blk, _) = blk_chan.recv().unwrap();
        let serialize = serialize(&blk);
        let ser_str = std::str::from_utf8(&serialize).unwrap();
        json_utils::add_str(&mut response, "block", ser_str);
        Ok(response)
    }

    fn sync_chain_info(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error> {
        match self.handler.get_tip() {
            Ok(Tip { height, .. }) => {
                let mut resp = json_utils::init_payload();
                let height: i64 = height.to_be().try_into().unwrap();
                json_utils::add_number(&mut resp, "headercount", height);
                json_utils::add_number(&mut resp, "blockcount", height);
                json_utils::add_str(&mut resp, "chain", self.network.as_str());

                // FIXME: need to be supported
                json_utils::add_bool(&mut resp, "ibd", false);
                Ok(resp)
            }
            Err(err) => Err(PluginError::new(1, err.to_string().as_str(), None)),
        }
    }

    /// The plugin, if fee estimation succeeds, must respond with the following fields:
    /// - opening (number), used for funding and also misc transactions
    /// - mutual_close (number), used for the mutual close transaction
    /// - unilateral_close (number), used for unilateral close (/commitment) transactions
    /// - delayed_to_us (number), used for resolving our output from our unilateral close
    /// - htlc_resolution (number), used for resolving HTLCs after an unilateral close
    /// - penalty (number), used for resolving revoked transactions
    /// - min_acceptable (number), used as the minimum acceptable feerate
    /// - max_acceptable (number), used as the maximum acceptable feerate
    fn sync_estimate_feed(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }

    fn sync_get_utxo(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut Plugin<T>,
        tx: &str,
        _: bool,
    ) -> Result<Value, Self::Error> {
        let tx: Transaction = deserialize(tx.as_bytes()).unwrap();
        let mut resp = json_utils::init_payload();
        if let Err(err) = self.handler.submit_transaction(tx) {
            json_utils::add_bool(&mut resp, "success", false);
            json_utils::add_str(&mut resp, "errmsg", format!("{}", err).as_str());
        } else {
            json_utils::add_bool(&mut resp, "success", true);
        }
        Ok(resp)
    }
}
