//! Lampo chain backend.
//!
//! Lampo does not expose a Bitcoin RPC port. A running daemon binds
//! `LampoHost` on `127.0.0.1:<port>` and logs `plugin host grpc`. This
//! client makes that unary call over cleartext HTTP/2.
//!
//! `--lampo-socket` is that address. The chain methods are the ones the
//! lampo bitcoind plugin registers. `LampoHost` today forwards lampo
//! commands, not those plugin methods, so a call fails until lampo routes
//! them. The wire format is already the one that route will use.
#![deny(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};

use folgore_common::client::fee_estimator::{FeeEstimator, FeePriority, FEE_RATES};
use folgore_common::client::FolgoreBackend;
use folgore_common::cln::json_utils;
use folgore_common::cln::plugin::error;
use folgore_common::cln::plugin::errors::PluginError;
use folgore_common::prelude::log;

const HOST_METHOD: &str = "/lampo.plugin.v1.LampoHost/Call";

pub struct Lampo {
    addr: String,
}

impl Lampo {
    pub fn new(addr: &str) -> Result<Self, PluginError> {
        let addr = addr.trim().trim_start_matches("http://").to_owned();
        if !addr.contains(':') {
            return Err(error!(
                "lampo host must be `127.0.0.1:<port>` (the plugin host grpc address)"
            ));
        }
        Ok(Self { addr })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, PluginError> {
        let params_json = serde_json::to_string(&params).map_err(|err| error!("{err}"))?;
        let payload = encode_rpc(method, &params_json);
        let response = h2_unary(&self.addr, HOST_METHOD, &payload)?;
        let (result_json, error_message) = decode_response(&response)?;
        if !error_message.is_empty() {
            return Err(error!("lampo `{method}`: {error_message}"));
        }
        if result_json.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&result_json).map_err(|err| error!("lampo result: {err}"))
    }
}

fn encode_rpc(method: &str, params_json: &str) -> Vec<u8> {
    let mut out = Vec::new();
    write_string(&mut out, 1, method);
    write_string(&mut out, 2, params_json);
    out
}

fn decode_response(payload: &[u8]) -> Result<(String, String), PluginError> {
    let mut result = String::new();
    let mut error_message = String::new();
    for (field, text) in strings(payload)? {
        match field {
            1 => result = text,
            2 => error_message = text,
            _ => {}
        }
    }
    Ok((result, error_message))
}

fn strings(payload: &[u8]) -> Result<Vec<(usize, String)>, PluginError> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < payload.len() {
        let key = read_varint(payload, &mut pos)?;
        let field = key >> 3;
        match key & 0x7 {
            0 => {
                let _ = read_varint(payload, &mut pos)?;
            }
            2 => {
                let size = read_varint(payload, &mut pos)?;
                if pos + size > payload.len() {
                    return Err(error!("lampo response truncated"));
                }
                let text = std::str::from_utf8(&payload[pos..pos + size])
                    .map_err(|err| error!("{err}"))?
                    .to_owned();
                pos += size;
                out.push((field, text));
            }
            5 => pos += 4,
            1 => pos += 8,
            _ => return Err(error!("lampo response bad wire type")),
        }
    }
    Ok(out)
}

fn write_string(out: &mut Vec<u8>, field: u32, value: &str) {
    write_varint(out, (u64::from(field) << 3) | 2);
    write_varint(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
}

fn write_varint(out: &mut Vec<u8>, mut value: u64) {
    while value > 0x7f {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint(data: &[u8], pos: &mut usize) -> Result<usize, PluginError> {
    let mut value = 0usize;
    let mut shift = 0;
    while *pos < data.len() {
        let byte = data[*pos];
        *pos += 1;
        value |= ((byte & 0x7f) as usize) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift > 28 {
            return Err(error!("varint too long"));
        }
    }
    Err(error!("truncated varint"))
}

fn h2_unary(addr: &str, path: &str, payload: &[u8]) -> Result<Vec<u8>, PluginError> {
    let mut stream = TcpStream::connect(addr).map_err(|err| error!("lampo `{addr}`: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|err| error!("{err}"))?;
    stream
        .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
        .map_err(|err| error!("{err}"))?;
    write_frame(&mut stream, 0x4, 0, 0, &[])?;
    let first = read_frame(&mut stream)?;
    if first.typ == 0x4 && first.flags & 0x1 == 0 {
        write_frame(&mut stream, 0x4, 0x1, 0, &[])?;
    }
    write_frame(&mut stream, 0x1, 0x4, 1, &encode_headers(addr, path))?;
    let mut body = Vec::with_capacity(5 + payload.len());
    body.push(0);
    body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    body.extend_from_slice(payload);
    write_frame(&mut stream, 0x0, 0x1, 1, &body)?;

    let mut message = Vec::new();
    loop {
        let frame = read_frame(&mut stream)?;
        match frame.typ {
            0x4 if frame.flags & 0x1 == 0 => write_frame(&mut stream, 0x4, 0x1, 0, &[])?,
            0x6 if frame.flags & 0x1 == 0 => write_frame(&mut stream, 0x6, 0x1, 0, &frame.payload)?,
            0x0 => message.extend_from_slice(&frame.payload),
            0x7 => return Err(error!("lampo closed the connection")),
            _ => {}
        }
        if frame.typ == 0x1 && frame.flags & 0x1 != 0 && !message.is_empty() {
            break;
        }
    }
    if message.len() < 5 {
        return Err(error!("lampo returned an empty grpc message"));
    }
    let size = u32::from_be_bytes(message[1..5].try_into().map_err(|_| error!("len"))?) as usize;
    let end = 5 + size;
    if end > message.len() {
        return Err(error!("lampo grpc message truncated"));
    }
    Ok(message[5..end].to_vec())
}

struct Frame {
    typ: u8,
    flags: u8,
    payload: Vec<u8>,
}

fn write_frame(
    stream: &mut TcpStream,
    typ: u8,
    flags: u8,
    id: u32,
    payload: &[u8],
) -> Result<(), PluginError> {
    let mut hdr = [0u8; 9];
    let size = payload.len() as u32;
    hdr[0..3].copy_from_slice(&size.to_be_bytes()[1..]);
    hdr[3] = typ;
    hdr[4] = flags;
    hdr[5..9].copy_from_slice(&id.to_be_bytes());
    stream
        .write_all(&hdr)
        .and_then(|_| stream.write_all(payload))
        .map_err(|err| error!("lampo write: {err}"))
}

fn read_frame(stream: &mut TcpStream) -> Result<Frame, PluginError> {
    let mut hdr = [0u8; 9];
    stream
        .read_exact(&mut hdr)
        .map_err(|err| error!("lampo read: {err}"))?;
    let size = u32::from_be_bytes([0, hdr[0], hdr[1], hdr[2]]) as usize;
    let mut payload = vec![0u8; size];
    if size > 0 {
        stream
            .read_exact(&mut payload)
            .map_err(|err| error!("{err}"))?;
    }
    Ok(Frame {
        typ: hdr[3],
        flags: hdr[4],
        payload,
    })
}

fn encode_headers(authority: &str, path: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0x83); // :method POST
    out.push(0x86); // :scheme http
    literal(&mut out, ":path", path);
    literal(&mut out, ":authority", authority);
    literal(&mut out, "content-type", "application/grpc");
    literal(&mut out, "te", "trailers");
    out
}

fn literal(out: &mut Vec<u8>, name: &str, value: &str) {
    out.push(0x00);
    hpack_string(out, name);
    hpack_string(out, value);
}

fn hpack_string(out: &mut Vec<u8>, text: &str) {
    out.push(text.len() as u8);
    out.extend_from_slice(text.as_bytes());
}

impl<T: Clone> FolgoreBackend<T> for Lampo {
    fn kind(&self) -> folgore_common::client::BackendKind {
        folgore_common::client::BackendKind::Lampo
    }

    fn sync_block_by_height(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        height: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let hash = self.call("getblockhash", json!([height]))?;
        let hash = hash
            .as_str()
            .ok_or_else(|| error!("getblockhash was not a string"))?;
        let block = self.call("getblock", json!([hash, 0]))?;
        let block = block
            .as_str()
            .ok_or_else(|| error!("getblock was not hex"))?;
        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "blockhash", hash);
        json_utils::add_str(&mut response, "block", block);
        Ok(response)
    }

    fn sync_chain_info(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        _: Option<u64>,
    ) -> Result<serde_json::Value, PluginError> {
        let info = self.call("getblockchaininfo", json!([]))?;
        let chain = info
            .get("chain")
            .and_then(|v| v.as_str())
            .unwrap_or("main");
        let chain = match chain {
            "main" | "bitcoin" => "main",
            "test" | "testnet" => "test",
            other => other,
        };
        let height = info.get("blocks").and_then(|v| v.as_i64()).unwrap_or(0);
        let ibd = info
            .get("initialblockdownload")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "chain", chain);
        json_utils::add_number(&mut response, "headercount", height);
        json_utils::add_number(&mut response, "blockcount", height);
        json_utils::add_bool(&mut response, "ibd", ibd);
        Ok(response)
    }

    fn sync_estimate_fees(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, PluginError> {
        let mut fee_map = BTreeMap::new();
        let mempool = self.call("getmempoolinfo", json!([]))?;
        let floor_btc = mempool
            .get("mempoolminfee")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.00001);
        fee_map.insert(0, (floor_btc * 100_000_000.0) as u64);
        for FeePriority(block, _) in FEE_RATES.iter().cloned() {
            let estimate = self.call("estimatesmartfee", json!([block]))?;
            let Some(rate) = estimate.get("feerate").and_then(|v| v.as_f64()) else {
                log::warn!("lampo estimatesmartfee {block} had no feerate");
                return FeeEstimator::null_estimate_fees();
            };
            fee_map.insert(u64::from(block), (rate * 100_000_000.0) as u64);
        }
        FeeEstimator::build_estimate_fees(&fee_map)
    }

    fn sync_get_utxo(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        txid: &str,
        idx: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let tx = self.call("getrawtransaction", json!([txid, true]))?;
        let vout = tx
            .get("vout")
            .and_then(|v| v.as_array())
            .ok_or_else(|| error!("getrawtransaction had no vout"))?;
        let output = vout
            .get(idx as usize)
            .ok_or_else(|| error!("vout {idx} missing"))?;
        let amount_btc = output.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let script = output
            .pointer("/scriptPubKey/hex")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut resp = json_utils::init_payload();
        json_utils::add_number(&mut resp, "amount", (amount_btc * 100_000_000.0) as i64);
        json_utils::add_str(&mut resp, "script", script);
        Ok(resp)
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut folgore_common::cln::plugin::plugin::Plugin<T>,
        tx: &str,
        _: bool,
    ) -> Result<serde_json::Value, PluginError> {
        let mut resp = json_utils::init_payload();
        match self.call("sendrawtransaction", json!([tx])) {
            Ok(_) => json_utils::add_bool(&mut resp, "success", true),
            Err(err) => {
                json_utils::add_bool(&mut resp, "success", false);
                json_utils::add_str(&mut resp, "errmsg", &err.to_string());
            }
        }
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_request_fields() {
        let payload = encode_rpc("getblockchaininfo", "[]");
        let fields = strings(&payload).expect("fields");
        assert_eq!(fields[0], (1, "getblockchaininfo".to_owned()));
        assert_eq!(fields[1], (2, "[]".to_owned()));
    }
}
