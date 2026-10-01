//! Future client interface definition.
pub mod fee_estimator;

use std::fmt;

use serde_json::Value;

use clightningrpc_plugin::error;
use clightningrpc_plugin::{errors::PluginError, plugin::Plugin};

pub enum BackendKind {
    Nakamoto,
    Esplora,
    BitcoinCore,
    Electrum,
}

impl TryFrom<&str> for BackendKind {
    type Error = PluginError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "nakamoto" => Ok(Self::Nakamoto),
            "esplora" => Ok(Self::Esplora),
            "bitcoind" => Ok(Self::BitcoinCore),
            "electrum" => Ok(Self::Electrum),
            _ => Err(error!("client {value} not supported")),
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nakamoto => write!(f, "nakamoto"),
            Self::Esplora => write!(f, "esplora"),
            Self::BitcoinCore => write!(f, "bitcoind"),
            Self::Electrum => write!(f, "electrum"),
        }
    }
}

/// Future backend trait that implement an optional async and sync
/// interface to work with a cln node that want access to a bitcoin
/// blockchain.
pub trait FolgoreBackend<T: Clone> {
    /// Return the Backend Kind
    fn kind(&self) -> BackendKind;
    /// The plugin must respond to getchaininfo with the following fields:
    /// - `chain` (string), the network name as introduced in bip70
    /// - `headercount` (number), the number of fetched block headers
    /// - `blockcount` (number), the number of fetched block body
    /// - `ibd` (bool), whether the backend is performing initial block download
    fn sync_chain_info(&self, _: &mut Plugin<T>, _: Option<u64>) -> Result<Value, PluginError>;

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
    fn sync_estimate_fees(&self, _: &mut Plugin<T>) -> Result<Value, PluginError>;

    /// This call takes one parameter, height, which determines the block height of the block to fetch.
    /// The plugin must set all fields to null if no block was found at the specified height.
    ///
    /// The plugin must respond to getrawblockbyheight with the following fields:
    /// - `blockhash` (string), the block hash as a hexadecimal string
    /// - `block` (string), the block content as a hexadecimal string
    fn sync_block_by_height(&self, _: &mut Plugin<T>, height: u64) -> Result<Value, PluginError>;

    /// This call takes two parameter, the txid (string) and the vout (number) identifying the UTXO we’re interested in.
    ///
    /// The plugin must set both fields to null if the specified TXO was spent.
    ///
    /// The plugin must respond to gettxout with the following fields:
    /// - amount (number), the output value in sats
    /// - script (string), the output scriptPubKey
    fn sync_get_utxo(&self, _: &mut Plugin<T>, _: &str, _: u64) -> Result<Value, PluginError>;

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
    ) -> Result<Value, PluginError>;

    /// FIXME: document this dev command if will be merged
    fn sync_dev_updateutxo(&self, _: &mut Plugin<T>, _: bool) -> Result<Value, PluginError> {
        Err(error!("unsupported `dev_updateutxo`"))
    }

    /// Lampo chain methods. Default is unsupported so a CLN-only backend
    /// still compiles. A backend that lampo spawns overrides these.
    fn chain_tip(&self) -> Result<Value, PluginError> {
        Err(error!("chain_tip is not implemented for this backend"))
    }

    fn chain_header(&self, _hash: &str) -> Result<Value, PluginError> {
        Err(error!("chain_header is not implemented for this backend"))
    }

    fn chain_block(&self, _hash: &str, _verbosity: u64) -> Result<Value, PluginError> {
        Err(error!("chain_block is not implemented for this backend"))
    }

    fn chain_fee(&self, _blocks: u64) -> Result<Value, PluginError> {
        Err(error!("chain_fee is not implemented for this backend"))
    }

    fn chain_mempool(&self) -> Result<Value, PluginError> {
        Err(error!("chain_mempool is not implemented for this backend"))
    }

    fn chain_broadcast(&self, _tx: &str) -> Result<Value, PluginError> {
        Err(error!(
            "chain_broadcast is not implemented for this backend"
        ))
    }
}
