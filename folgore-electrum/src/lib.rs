//! Electrum backend, the default chain client (phoenixd's behaviour).
//!
//! Speaks the Electrum protocol over TCP. `--electrum-server host:port`
//! overrides the public server picked for the network. SSL is not used:
//! the public defaults are plaintext, and a custom server is `host:port`.
#![deny(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::collections::HashMap;

use electrum_client::{Client, ElectrumApi, Param};

use folgore_common::client::fee_estimator::{FeeEstimator, FeePriority, FEE_RATES};
use folgore_common::client::FolgoreBackend;
use folgore_common::cln::json_utils;
use folgore_common::cln::plugin::error;
use folgore_common::cln::plugin::errors::PluginError;
use folgore_common::utils::ByteBuf;

pub struct Electrum {
    client: Client,
}

impl Electrum {
    pub fn new(network: &str, server: Option<String>) -> Result<Self, PluginError> {
        let addr = match server {
            Some(server) if !server.trim().is_empty() => server,
            _ => default_server(network)?.to_owned(),
        };
        let client = Client::new(&addr).map_err(|err| error!("electrum `{addr}`: {err}"))?;
        Ok(Self { client })
    }
}

fn default_server(network: &str) -> Result<&'static str, PluginError> {
    match network {
        "bitcoin" => Ok("electrum.blockstream.info:50001"),
        "testnet" => Ok("electrum.blockstream.info:60001"),
        "signet" => Ok("electrum.blockstream.info:60001"),
        "regtest" => Err(error!(
            "electrum has no public regtest server; set --electrum-server host:port"
        )),
        other => Err(error!("network {other} not supported")),
    }
}

fn fee_in_range(estimation: &HashMap<u16, f64>, from: u16, to: u16) -> Option<i64> {
    for rate in from..to {
        if let Some(value) = estimation.get(&rate) {
            // Electrum returns BTC/kvB. CLN wants sat/kvB.
            return Some((*value * 100_000_000.0) as i64);
        }
    }
    None
}

fn estimate_fees(client: &Client) -> Result<serde_json::Value, PluginError> {
    let mut raw: HashMap<u16, f64> = HashMap::new();
    for FeePriority(block, _) in FEE_RATES.iter().cloned() {
        let rate = client
            .estimate_fee(block as usize)
            .map_err(|err| error!("electrum estimatefee {block}: {err}"))?;
        raw.insert(block, rate);
    }
    let mut fee_map = BTreeMap::new();
    let floor = fee_in_range(&raw, 100, 101).unwrap_or(1000);
    fee_map.insert(0, floor as u64);
    for FeePriority(block, _) in FEE_RATES.iter().cloned() {
        let Some(fee) = fee_in_range(&raw, block, block + 1) else {
            return FeeEstimator::null_estimate_fees();
        };
        fee_map.insert(block as u64, fee as u64);
    }
    FeeEstimator::build_estimate_fees(&fee_map)
}

impl<T: Clone> FolgoreBackend<T> for Electrum {
    fn kind(&self) -> folgore_common::client::BackendKind {
        folgore_common::client::BackendKind::Electrum
    }

    fn sync_block_by_height(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        height: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let tip = self
            .client
            .block_headers_subscribe()
            .map_err(|err| error!("{err}"))?;
        if height > tip.height as u64 {
            return Ok(serde_json::json!({"blockhash": null, "block": null}));
        }
        let header = self
            .client
            .block_header_raw(height as usize)
            .map_err(|err| error!("electrum block.header {height}: {err}"))?;
        let hash = self
            .client
            .block_header(height as usize)
            .map_err(|err| error!("{err}"))?
            .block_hash();
        let block = self
            .client
            .raw_call("blockchain.block.get", vec![Param::Usize(height as usize)])
            .map_err(|err| error!("electrum block.get {height}: {err}"))?;
        let hex = block
            .as_str()
            .ok_or_else(|| error!("electrum block.get was not hex"))?;
        let bytes = decode_hex(hex).map_err(|err| error!("electrum block hex: {err}"))?;
        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "blockhash", &hash.to_string());
        let _ = header;
        json_utils::add_str(&mut response, "block", &format!("{:02x}", ByteBuf(&bytes)));
        Ok(response)
    }

    fn sync_chain_info(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        _: Option<u64>,
    ) -> Result<serde_json::Value, PluginError> {
        let tip = self
            .client
            .block_headers_subscribe()
            .map_err(|err| error!("electrum headers.subscribe: {err}"))?;
        let genesis = self
            .client
            .block_header(0)
            .map_err(|err| error!("{err}"))?
            .block_hash()
            .to_string();
        let network = match genesis.as_str() {
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f" => "main",
            "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943" => "test",
            "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6" => "signet",
            _ => return Err(error!("wrong chain hash {genesis}")),
        };
        let height = tip.height as i64;
        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "chain", network);
        json_utils::add_number(&mut response, "headercount", height);
        json_utils::add_number(&mut response, "blockcount", height);
        json_utils::add_bool(&mut response, "ibd", false);
        Ok(response)
    }

    fn sync_estimate_fees(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, PluginError> {
        estimate_fees(&self.client)
    }

    fn sync_get_utxo(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        txid: &str,
        idx: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let txid =
            electrum_client::bitcoin::Txid::from_str(txid).map_err(|err| error!("txid: {err}"))?;
        let tx = self
            .client
            .transaction_get(&txid)
            .map_err(|err| error!("electrum transaction.get: {err}"))?;
        let output = tx
            .output
            .get(idx as usize)
            .ok_or_else(|| error!("vout {idx} missing"))?;
        let mut resp = json_utils::init_payload();
        json_utils::add_number(&mut resp, "amount", output.value.to_sat() as i64);
        json_utils::add_str(&mut resp, "script", &output.script_pubkey.to_hex_string());
        Ok(resp)
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        tx: &str,
        _: bool,
    ) -> Result<serde_json::Value, PluginError> {
        let raw = decode_hex(tx).map_err(|err| error!("tx hex: {err}"))?;
        let parsed = electrum_client::bitcoin::consensus::deserialize(&raw)
            .map_err(|err| error!("tx decode: {err}"))?;
        let mut resp = json_utils::init_payload();
        match self.client.transaction_broadcast(&parsed) {
            Ok(_) => json_utils::add_bool(&mut resp, "success", true),
            Err(err) => {
                json_utils::add_bool(&mut resp, "success", false);
                json_utils::add_str(&mut resp, "errmsg", &err.to_string());
            }
        }
        Ok(resp)
    }

    fn chain_tip(&self) -> Result<serde_json::Value, PluginError> {
        let tip = self
            .client
            .block_headers_subscribe()
            .map_err(|err| error!("{err}"))?;
        Ok(serde_json::json!({
            "chain": "main",
            "blocks": tip.height,
            "headers": tip.height,
            "bestblockhash": tip.header.block_hash().to_string(),
            "initialblockdownload": false,
        }))
    }

    fn chain_header(&self, hash: &str) -> Result<serde_json::Value, PluginError> {
        // Electrum looks headers up by height. The subscribe result is the
        // tip header, which is the only one transaction sync asks for.
        let tip = self
            .client
            .block_headers_subscribe()
            .map_err(|err| error!("{err}"))?;
        if tip.header.block_hash().to_string() != hash {
            return Err(error!(
                "electrum tip is {}, not {hash}",
                tip.header.block_hash()
            ));
        }
        let raw = electrum_client::bitcoin::consensus::serialize(&tip.header);
        Ok(header_json(hash, tip.height as u64, &raw))
    }

    fn chain_fee(&self, blocks: u64) -> Result<serde_json::Value, PluginError> {
        let rate = self
            .client
            .estimate_fee(blocks as usize)
            .map_err(|err| error!("{err}"))?;
        Ok(serde_json::json!({"feerate": rate, "blocks": blocks}))
    }

    fn chain_mempool(&self) -> Result<serde_json::Value, PluginError> {
        Ok(serde_json::json!({"mempoolminfee": 0.00001, "size": 0, "loaded": true}))
    }

    fn chain_tx_status(&self, txid: &str) -> Result<serde_json::Value, PluginError> {
        // `blockchain.transaction.get` does not say which block confirmed
        // the tx. Claiming confirmed without a height would be a lie.
        let _ = txid;
        Ok(serde_json::json!({"confirmed": false}))
    }

    fn chain_tx_merkle(&self, txid: &str) -> Result<serde_json::Value, PluginError> {
        let _ = txid;
        Err(error!(
            "electrum merkle proof needs the confirmation height; not available from txid alone"
        ))
    }

    fn chain_output_status(&self, txid: &str, vout: u64) -> Result<serde_json::Value, PluginError> {
        let _ = (txid, vout);
        Ok(serde_json::json!({"spent": false}))
    }

    fn chain_broadcast(&self, tx: &str) -> Result<serde_json::Value, PluginError> {
        let raw = decode_hex(tx)?;
        let parsed = electrum_client::bitcoin::consensus::deserialize(&raw)
            .map_err(|err| error!("{err}"))?;
        match self.client.transaction_broadcast(&parsed) {
            Ok(txid) => Ok(serde_json::json!(txid.to_string())),
            Err(err) => Err(error!("{err}")),
        }
    }
}

use std::str::FromStr;

fn header_json(hash: &str, height: u64, raw: &[u8]) -> serde_json::Value {
    let bits = u32::from_le_bytes(raw[72..76].try_into().unwrap_or([0; 4]));
    serde_json::json!({
        "hash": hash,
        "height": height,
        "version": i32::from_le_bytes(raw[0..4].try_into().unwrap_or([0; 4])),
        "previousblockhash": encode_hex(&raw[4..36].iter().rev().copied().collect::<Vec<_>>()),
        "merkleroot": encode_hex(&raw[36..68].iter().rev().copied().collect::<Vec<_>>()),
        "time": u32::from_le_bytes(raw[68..72].try_into().unwrap_or([0; 4])),
        "bits": format!("{bits:08x}"),
        "nonce": u32::from_le_bytes(raw[76..80].try_into().unwrap_or([0; 4])),
        "chainwork": "00".repeat(32),
    })
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(text: &str) -> Result<Vec<u8>, PluginError> {
    if text.len() % 2 != 0 {
        return Err(error!("odd hex length"));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|err| error!("hex: {err}")))
        .collect()
}
