//! Future client interface definition.
use clightningrpc_plugin::plugin::Plugin;
use serde_json::Value;
use std::collections::HashMap;

/// Future backend trait that implement an optional async and sync
/// interface to work with a cln node that want access to a bitcoin
/// blockchain.
pub trait FutureBackend<T: Clone> {
    type Error = String;

    /// The plugin must respond to getchaininfo with the following fields:
    /// - `chain` (string), the network name as introduced in bip70
    /// - `headercount` (number), the number of fetched block headers
    /// - `blockcount` (number), the number of fetched block body
    /// - `ibd` (bool), whether the backend is performing initial block download
    fn sync_chain_info(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error>;

    /// Polled by lightningd to get the current feerate, all values must
    /// be passed in sat/kVB.
    ///
    /// The plugin, if fee estimation succeeds, must respond with the following fields:
    /// - opening (number), used for funding and also misc transactions
    /// - mutual_close (number), used for the mutual close transaction
    /// - unilateral_close (number), used for unilateral close (/commitment) transactions
    /// - delayed_to_us (number), used for resolving our output from our unilateral close
    /// - htlc_resolution (number), used for resolving HTLCs after an unilateral close
    /// - penalty (number), used for resolving revoked transactions
    /// - min_acceptable (number), used as the minimum acceptable feerate
    /// - max_acceptable (number), used as the maximum acceptable feerate
    /// If fee estimation fails, the plugin must set all the fields to null.
    fn sync_estimate_fees(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error>;

    /// This call takes one parameter, height, which determines the block height of the block to fetch.
    /// The plugin must set all fields to null if no block was found at the specified height.
    ///
    /// The plugin must respond to getrawblockbyheight with the following fields:
    /// - `blockhash` (string), the block hash as a hexadecimal string
    /// - `block` (string), the block content as a hexadecimal string
    fn sync_block_by_height(&self, _: &mut Plugin<T>, height: u64) -> Result<Value, Self::Error>;

    fn sync_get_utxo(&self, _: &mut Plugin<T>) -> Result<Value, Self::Error>;

    /// This call takes two parameters, a string `tx` representing a hex-encoded
    /// Bitcoin transaction, and a boolean `allowhighfees`, which if set means
    /// suppress any high-fees check implemented in the backend,
    /// since the given transaction may have fees that are very high.
    ///
    /// The plugin must broadcast it and respond with the following fields:
    /// - `success` (boolean), which is true if the broadcast succeeded
    /// - `errmsg` (string), if success is false, the reason why it failed
    fn sync_send_raw_transaction(
        &self,
        _: &mut Plugin<T>,
        _: &str,
        _: bool,
    ) -> Result<Value, Self::Error>;
}
