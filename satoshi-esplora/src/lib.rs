use std::fmt::Display;

use clightningrpc_common::json_utils;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::types::LogLevel;
use esplora_client::api::{FromHex, Transaction, TxOut, Txid};
use esplora_client::deserialize;

use esplora_client::{BlockingClient, Builder};
use satoshi_common::client::SatoshiBackend;
use serde_json::json;

#[derive(Clone)]
enum Network {
    Bitcoin(String),
    Testnet(String),
    Liquid(String),
    BitcoinTor(String),
    TestnetTor(String),
    LiquidTor(String),
}

impl Network {
    pub fn url(&self) -> String {
        match &self {
            Self::Bitcoin(url) => url.to_string(),
            Self::Liquid(url) => url.to_string(),
            Self::Testnet(url) => url.to_string(),
            Self::BitcoinTor(url) => url.to_string(),
            Self::TestnetTor(url) => url.to_string(),
            Self::LiquidTor(url) => url.to_string(),
        }
    }
}

impl TryFrom<&str> for Network {
    type Error = PluginError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "bitcoin" => Ok(Self::Bitcoin("https://blockstream.infoi/api".to_owned())),
            "bitcoin/tor" => Ok(Self::BitcoinTor(
                "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion/api"
                    .to_owned(),
            )),
            "testnet" => Ok(Self::Testnet(
                "https://blockstream.info/testnet/api".to_owned(),
            )),
            "testnet/tor" => Ok(Self::TestnetTor(
                "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion/testnet/api"
                    .to_owned(),
            )),
            _ => Err(PluginError::new(
                -1,
                &format!("network {value} not supported"),
                None,
            )),
        }
    }
}

// FIXME: move this inside the Plugin API to map the error
/// convert the error to a plugin error
fn from<T: Display>(value: T) -> PluginError {
    PluginError::new(-1, &format!("{value}"), None)
}

#[derive(Clone)]
pub struct Esplora {
    network: Network,
    client: BlockingClient,
}

impl Esplora {
    pub fn new(network: &str) -> Result<Self, PluginError> {
        let network = Network::try_from(network)?;
        let builder = Builder::new(&network.url());
        Ok(Self {
            network,
            client: builder.build_blocking().unwrap(),
        })
    }
}

impl<T: Clone> SatoshiBackend<T> for Esplora {
    fn sync_block_by_height(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        height: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let block = self
            .client
            .get_blocks(Some(height.try_into().unwrap()))
            .map_err(from)?;
        let block = block.first().clone().unwrap();
        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "blockhash", &block.id.to_string());
        json_utils::add_number(&mut response, "height", height.try_into().unwrap());
        json_utils::add_number(
            &mut response,
            "time",
            block.time.timestamp.try_into().unwrap(),
        );
        Ok(response)
    }

    fn sync_chain_info(
        &self,
        plugin: &mut clightningrpc_plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, PluginError> {
        let current_height = self.client.get_height().map_err(from)?;
        plugin.log(
            LogLevel::Info,
            &format!("blockchain height: {current_height}"),
        );
        let genesis = self.client.get_blocks(Some(0)).map_err(from)?;

        let genesis = genesis.first().clone().unwrap();
        let network = match genesis.id.to_string().as_str() {
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f" => "main",
            "000000000933ea01ad0ee984209779baaec3ced90fa3f408719526f8d77f4943" => "test",
            "1466275836220db2944ca059a3a10ef6fd2ea684b0688d2c379296888a206003" => "liquidv1",
            _ => panic!(""),
        };

        let mut response = json_utils::init_payload();
        json_utils::add_str(&mut response, "chain", network);
        json_utils::add_number(&mut response, "headercount", current_height.into());
        json_utils::add_number(&mut response, "blockcount", current_height.into());
        json_utils::add_bool(&mut response, "ibd", false);
        Ok(response)
    }

    fn sync_estimate_fees(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, PluginError> {
        let fee = self.client.get_fee_estimates().map_err(from)?;

        let hight = fee["6"] as i64;
        let urgent = fee["6"] as i64;
        let normal = fee["12"] as i64;
        let slow = fee["100"] as i64;

        // FIXME: manage to return an empty response when there is some error
        let mut resp = json_utils::init_payload();
        json_utils::add_number(&mut resp, "opening", normal);
        json_utils::add_number(&mut resp, "mutual_close", slow);
        json_utils::add_number(&mut resp, "unilateral_close", urgent);
        json_utils::add_number(&mut resp, "delayed_to_us", normal);
        json_utils::add_number(&mut resp, "htlc_resolution", urgent);
        json_utils::add_number(&mut resp, "penalty", normal);
        json_utils::add_number(&mut resp, "min_acceptable", slow / 2);
        json_utils::add_number(&mut resp, "max_acceptable", hight * 10);
        Ok(resp)
    }

    fn sync_get_utxo(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        txid: &str,
        idx: u64,
    ) -> Result<serde_json::Value, PluginError> {
        let txid = Txid::from_hex(txid).map_err(from)?;
        let utxo = self.client.get_tx(&txid).map_err(from)?;

        let mut resp = json_utils::init_payload();
        if let Some(tx) = utxo {
            let output: TxOut = tx.output[idx as usize].clone();
            json_utils::add_number(&mut resp, "amount", output.value.try_into().map_err(from)?);
            // FIXME: the to string here is what we are looking for?
            json_utils::add_str(&mut resp, "script", &output.script_pubkey.to_string());
            return Ok(resp);
        }
        // FIXME: return a null response, this requires some hep from the cln API side
        Ok(json! {{}})
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        tx: &str,
        _with_hight_fee: bool,
    ) -> Result<serde_json::Value, PluginError> {
        let tx: Transaction = deserialize(tx.as_bytes()).map_err(from)?;
        let tx_send = self.client.broadcast(&tx);

        let mut resp = json_utils::init_payload();
        json_utils::add_bool(&mut resp, "success", tx_send.is_ok());
        if let Err(err) = tx_send {
            json_utils::add_str(&mut resp, "errmsg", &err.to_string());
        }
        Ok(resp)
    }
}
