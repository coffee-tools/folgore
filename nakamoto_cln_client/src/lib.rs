//! Future client implementation for nakamoto
use clightningrpc_plugin::commands::json_utils;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use future_common::client::FutureBackend;
use nakamoto_client::{Client, Config, Error, Handle};
use nakamoto_net_poll::{Reactor, Waker};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::TcpStream;
use std::thread::JoinHandle;

struct Nakamoto {
    handler: Handle<Waker>,
    worker: JoinHandle<Result<(), Error>>,
}

impl Nakamoto {
    pub fn new(config: Config) -> Result<Self, Error> {
        let nakamoto = Client::<Reactor<TcpStream>>::new()?;
        let handler = nakamoto.handle();
        let worker = std::thread::spawn(|| nakamoto.run(config));
        let client = Nakamoto { handler, worker };

        Ok(client)
    }

    // FIXME: return the correct error
    pub fn stop(self) -> Result<(), Error> {
        let _ = self.worker.join();
        Ok(())
    }
}

impl<T: Clone> FutureBackend<T> for Nakamoto {
    type Error = PluginError;

    /// The plugin must respond to getrawblockbyheight with the following fields:
    /// - blockhash (string), the block hash as a hexadecimal string
    /// - block (string), the block content as a hexadecimal string
    fn sync_block_by_height(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error> {
        let mut response = json_utils::init_payload();
        let header = self.handler.get_block_by_height(0).unwrap();
        if let None = header {
            return Ok(response);
        }
        let header = header.unwrap();
        json_utils::add_str(
            &mut response,
            "blockhash",
            header.block_hash().to_string().as_str(),
        );
        // TODO: get block in nakamoto
        json_utils::add_str(&mut response, "block", "");
        Ok(response)
    }

    /// The plugin must respond to getchaininfo with the following fields:
    /// - chain (string), the network name as introduced in bip70
    /// - headercount (number), the number of fetched block headers
    /// - blockcount (number), the number of fetched block body
    /// - ibd (bool), whether the backend is performing initial block download
    fn sync_chain_info(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error> {
        todo!()
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

    /// The plugin must respond to gettxout with the following fields:
    /// - amount (number), the output value in sats
    /// - script (string), the output scriptPubKey
    ///
    fn sync_get_utxo(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }

    /// The plugin must respond to gettxout with the following fields:
    /// - amount (number), the output value in sats
    /// - script (string), the output scriptPubKey
    ///
    /// The plugin must broadcast it and respond with the following fields:
    /// - success (boolean), which is true if the broadcast succeeded
    /// - errmsg (string), if success is false, the reason why it failed
    fn sync_send_raw_transaction(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }
}
