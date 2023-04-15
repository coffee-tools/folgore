//! Plugin definition.
use std::str::FromStr;
use std::sync::Arc;

use clightningrpc_plugin::commands::{types::CLNConf, RPCCommand};
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use clightningrpc_plugin::types::LogLevel;
use serde_json::{json, Value};

use folgore_common::client::FolgoreBackend;
use folgore_esplora::Esplora;
use folgore_nakamoto::{Config, Nakamoto, Network};

use crate::model::{BlockByHeight, GetChainInfo, GetUTxo, SendRawTx};

pub(crate) enum ClientType {
    Nakamoto,
    Esplora,
}

impl TryFrom<&str> for ClientType {
    type Error = PluginError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "nakamoto" => Ok(Self::Nakamoto),
            "esplora" => Ok(Self::Esplora),
            _ => Err(PluginError::new(
                -1,
                &format!("client {value} not supported"),
                None,
            )),
        }
    }
}

#[derive(Clone)]
pub struct PluginState {
    pub(crate) client: Option<Arc<dyn FolgoreBackend<PluginState>>>,
    pub(crate) esplora_url: Option<String>,
}

impl PluginState {
    fn new() -> Self {
        PluginState {
            client: None,
            esplora_url: None,
        }
    }

    fn new_client(&mut self, client: &str, conf: &CLNConf) -> Result<(), PluginError> {
        let client = ClientType::try_from(client)?;
        match client {
            ClientType::Nakamoto => {
                let mut config = Config::default();
                config.network = Network::from_str(&conf.network).unwrap();
                let client = Nakamoto::new(config).map_err(|err| error!("{err}"))?;
                self.client = Some(Arc::new(client));
                Ok(())
            }
            ClientType::Esplora => {
                // FIXME: check if there is the proxy enabled to pass the tor addrs
                let client = Esplora::new(&conf.network, self.esplora_url.to_owned())?;
                self.client = Some(Arc::new(client));
                Ok(())
            }
        }
    }
}

pub fn build_plugin() -> Plugin<PluginState> {
    let plugin = Plugin::new(PluginState::new(), false)
        .add_opt(
            "bitcoin-rpcpassword",
            "string",
            None,
            "Bitcoin RPC password",
            false,
        )
        .add_opt("bitcoin-rpcuser", "string", None, "Bitcoin RPC use", false)
        .add_opt(
            "bitcoin-client",
            "string",
            Some("esplora".to_owned()),
            "Set up the client to use",
            false,
        )
        .add_opt(
            "bitcoin-esplora-url",
            "string",
            Some(String::new()),
            "A custom esplora backend url where to fetch the bitcoin data",
            false,
        )
        .add_rpc_method(
            "getchaininfo",
            "",
            "getchaininfo to fetch information the data from the client",
            GetChainInfoRPC {},
        )
        .add_rpc_method(
            "estimatefees",
            "",
            "estimatefees to fetch the feed estimation from the client",
            EstimateFeesRPC {},
        )
        .add_rpc_method(
            "getrawblockbyheight",
            "",
            "getrawblockbyheight to fetch the raw block by height",
            GetRawBlockByHeightRPC {},
        )
        .add_rpc_method(
            "getutxout",
            "",
            "getutxout to fetch a utx with {txid} and {vout}",
            GetUtxOutRPC {},
        )
        .add_rpc_method(
            "sendrawtransaction",
            "",
            "sendrawtransaction to publish a new transaction",
            SendRawTransactionRPC {},
        )
        .on_init(on_init)
        .to_owned();
    plugin
}

fn on_init(plugin: &mut Plugin<PluginState>) -> Value {
    let client: String = plugin.get_opt("bitcoin-client").unwrap();
    let esplora_url: Option<String> = plugin.get_opt("bitcoin-esplora-url").unwrap();
    if let Some(url) = esplora_url {
        if !url.trim().is_empty() {
            plugin.state.esplora_url = Some(url.trim().to_string());
        }
    }

    let conf = plugin.configuration.clone().unwrap();
    if let Err(err) = plugin.state.new_client(&client, &conf) {
        plugin.log(LogLevel::Debug, &format!("{err}"));
    };
    json!({})
}

// FIXME use the plugin_macros to semplify all this code
#[derive(Clone)]
struct GetChainInfoRPC {}

impl RPCCommand<PluginState> for GetChainInfoRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        plugin.log(LogLevel::Info, &format!("cln request {request}"));
        let request: GetChainInfo = serde_json::from_value(request)?;
        let result = client.sync_chain_info(plugin, request.height);
        plugin.log(LogLevel::Debug, &format!("{:?}", result));
        result
    }
}

#[derive(Clone)]
struct EstimateFeesRPC {}

impl RPCCommand<PluginState> for EstimateFeesRPC {
    fn call<'c>(&self, plugin: &mut Plugin<PluginState>, _: Value) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call estimate fee info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        let result = client.sync_estimate_fees(plugin);
        plugin.log(LogLevel::Debug, &format!("{:?}", result));
        result
    }
}

#[derive(Clone)]
struct GetRawBlockByHeightRPC {}

impl RPCCommand<PluginState> for GetRawBlockByHeightRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get block by height");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        plugin.log(LogLevel::Info, &format!("cln request {request}"));
        let request: BlockByHeight = serde_json::from_value(request)?;
        client.sync_block_by_height(plugin, request.height)
    }
}

#[derive(Clone)]
struct GetUtxOutRPC {}

impl RPCCommand<PluginState> for GetUtxOutRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get utxo");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        plugin.log(LogLevel::Info, &format!("cln request: {request}"));
        let request: GetUTxo = serde_json::from_value(request)?;
        let result = client.sync_get_utxo(plugin, &request.txid, request.vout);
        plugin.log(LogLevel::Debug, &format!("{:?}", result));
        result
    }
}

#[derive(Clone)]
struct SendRawTransactionRPC {}

impl RPCCommand<PluginState> for SendRawTransactionRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        plugin.log(LogLevel::Info, &format!("cln request: {request}"));
        let request: SendRawTx = serde_json::from_value(request)?;
        client.sync_send_raw_transaction(plugin, &request.tx, request.allowhighfees)
    }
}
