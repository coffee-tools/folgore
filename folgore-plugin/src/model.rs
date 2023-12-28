//! Rust model to unwrap the request send from core lightning
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct BlockByHeight {
    pub(crate) height: u64,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct GetUTxo {
    pub(crate) txid: String,
    pub(crate) vout: u64,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct SendRawTx {
    pub(crate) tx: String,
    pub(crate) allowhighfees: bool,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct GetChainInfo {
    pub(crate) last_height: Option<u64>,
}
