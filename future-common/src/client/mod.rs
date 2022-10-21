//! Future client interface definition.
use clightningrpc_plugin::plugin::Plugin;
use serde_json::Value;
use std::collections::HashMap;

/// Future backend trait that implement an optional async and sync
/// interface to work with a cln node that want access to a bitcoin
/// blockchain.
pub trait FutureBackend<T: Clone> {
    type Error = String;

    fn sync_chain_info(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error> {
        todo!()
    }

    fn sync_estimate_feed(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }

    fn sync_block_by_height(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error> {
        todo!()
    }

    fn sync_get_utxo(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }

    fn sync_send_raw_transaction(&self, _: &mut Plugin<T>) -> Result<(), Self::Error> {
        todo!()
    }
}
