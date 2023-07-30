//! Plugin definition.
use std::str::FromStr;
use std::sync::Arc;

use folgore_bitcoind::BitcoinCore;
use serde_json::{json, Value};

use clightningrpc_plugin::commands::{types::CLNConf, RPCCommand};
use clightningrpc_plugin::error;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use clightningrpc_plugin::types::LogLevel;
use clightningrpc_plugin_macros::plugin;
use clightningrpc_plugin_macros::rpc_method;

use folgore_common::client::FolgoreBackend;
use folgore_esplora::Esplora;
use folgore_nakamoto::{Config, Nakamoto, Network};

use crate::model::{BlockByHeight, GetChainInfo, GetUTxo, SendRawTx};

pub(crate) enum ClientType {
    Nakamoto,
    Esplora,
    BitcoinCore,
}

impl TryFrom<&str> for ClientType {
    type Error = PluginError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "nakamoto" => Ok(Self::Nakamoto),
            "esplora" => Ok(Self::Esplora),
            "bitcoind" => Ok(Self::BitcoinCore),
            _ => Err(error!("client {value} not supported")),
        }
    }
}

#[derive(Clone)]
pub struct PluginState {
    pub(crate) client: Option<Arc<dyn FolgoreBackend<PluginState>>>,
    pub(crate) fallback: Option<Arc<dyn FolgoreBackend<PluginState>>>,
    pub(crate) esplora_url: Option<String>,
    pub(crate) core_url: Option<String>,
    pub(crate) core_user: Option<String>,
    pub(crate) core_pass: Option<String>,
    pub(crate) _retry_strategy: Option<String>,
}

impl PluginState {
    fn new() -> Self {
        PluginState {
            client: None,
            fallback: None,
            esplora_url: None,
            core_url: None,
            core_pass: None,
            core_user: None,
            _retry_strategy: None,
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
            ClientType::BitcoinCore => {
                let client = BitcoinCore::new(
                    &self
                        .core_url
                        .clone()
                        .ok_or(error!("bitcoin url not specified"))?,
                    &self
                        .core_user
                        .clone()
                        .ok_or(error!("bitcoin user not specified"))?,
                    &self
                        .core_url
                        .clone()
                        .ok_or(error!("bitcoin pass not specied"))?,
                )?;
                self.client = Some(Arc::new(client));
                Ok(())
            }
        }
    }
}

pub fn build_plugin() -> Plugin<PluginState> {
    let mut plugin = plugin! {
        state: PluginState::new(),
        dynamic: false,
        notification: [],
        methods: [
           get_chain_info,
           estimate_fees,
           get_raw_block_by_height,
           getutxout,
            send_rawtransaction,

        ],
        hooks: [],
    };
    plugin
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
            "bitcoin-fallback-client",
            "string",
            Some("esplora".to_owned()),
            "Set up the client to use in case of fallback client (by default `esplora`)",
            false,
        )
        .add_opt(
            "bitcoin-esplora-url",
            "string",
            Some(String::new()),
            "A custom esplora backend url where to fetch the bitcoin data",
            false,
        )
        .on_init(on_init)
}

fn on_init(plugin: &mut Plugin<PluginState>) -> Value {
    let client: String = plugin.get_opt("bitcoin-client").unwrap();
    let esplora_url: Option<String> = plugin.get_opt("bitcoin-esplora-url").ok();
    if let Some(url) = esplora_url {
        if !url.trim().is_empty() {
            plugin.state.esplora_url = Some(url.trim().to_string());
        }
    }

    if let Some(url) = plugin.get_opt::<String>("bitcoin-rpcurl").ok() {
        if !url.trim().is_empty() {
            plugin.state.core_url = Some(url);
        }
    }

    if let Some(user) = plugin.get_opt::<String>("bitcoin-rpcuser").ok() {
        if !user.trim().is_empty() {
            plugin.state.core_user = Some(user);
        }
    }

    if let Some(pass) = plugin.get_opt::<String>("bitcoin-rpcpassword").ok() {
        if !pass.trim().is_empty() {
            plugin.state.core_pass = Some(pass);
        }
    }

    // SAFETY: the configuration should be always not null otherwise
    // there is a bug inside the plugin API
    let conf = plugin.configuration.clone().unwrap();
    if let Err(err) = plugin.state.new_client(&client, &conf) {
        return json!({
            "disable": format!("{err}"),
        });
    };

    if let Some(fallback) = plugin.get_opt::<String>("bitcoin-fallback-client").ok() {
        if !fallback.trim().is_empty() {
            if let Err(err) = plugin.state.new_client(&fallback, &conf) {
                return json!({
                    "disable": format!("{err}"),
                });
            };
        }
    }

    json!({})
}

#[rpc_method(
    rpc_name = "getchaininfo",
    description = "getchaininfo to fetch information the data from the client"
)]
fn get_chain_info(plugin: &mut Plugin<PluginState>, request: Value) -> Result<Value, PluginError> {
    plugin.log(LogLevel::Debug, "call get chain info");
    let mut plg = plugin.to_owned();
    let client = plg.state.client.as_mut().ok_or(error!(
        "Client must be not null at this point, please report a bug"
    ))?;
    // FIXME: make this not null
    let fallback = plg
        .state
        .fallback
        .as_mut()
        .ok_or(error!("Fallback must be not null"))?;
    plugin.log(LogLevel::Info, &format!("cln request {request}"));
    let request: GetChainInfo = serde_json::from_value(request)?;

    let mut result: Result<Value, PluginError> = Err(error!("result undefined"));
    for client in [client, fallback] {
        result = client.sync_chain_info(plugin, request.height.clone());
        let Ok(ref result) = result else {
            plugin.log(
                LogLevel::Warn,
                &format!(
                    "client (TODO add the name) return an error: {}",
                    result.clone().err().unwrap()
                ),
            );
            continue;
        };
        break;
    }
    plugin.log(LogLevel::Debug, &format!("{:?}", result));
    result
}

#[rpc_method(
    rpc_name = "estimatefees",
    description = "estimatefees to fetch the feed estimation from the client"
)]
fn estimate_fees(plugin: &mut Plugin<PluginState>, _: Value) -> Result<Value, PluginError> {
    plugin.log(LogLevel::Debug, "call estimate fee info");
    let client = plugin
        .state
        .client
        .clone()
        .ok_or(error!("client must be not null at this point"))?;
    let fallback = plugin
        .state
        .fallback
        .clone()
        .ok_or(error!("fallback backend must be not null"))?;
    let mut result: Result<Value, PluginError> = Err(error!("the result is null"));
    for client in [client, fallback] {
        result = client.sync_estimate_fees(plugin);
        let Ok(ref result) = result else {
            plugin.log(
                LogLevel::Warn,
                &format!(
                    "client (TODO add the name) return an error: {}",
                    result.clone().err().unwrap()
                ),
            );
            continue;
        };
        break;
    }
    plugin.log(LogLevel::Debug, &format!("{:?}", result));
    result
}

#[rpc_method(
    rpc_name = "getrawblockbyheight",
    description = "getrawblockbyheight to fetch the raw block by height"
)]
fn get_raw_block_by_height(
    plugin: &mut Plugin<PluginState>,
    request: Value,
) -> Result<Value, PluginError> {
    plugin.log(LogLevel::Debug, "call get block by height");
    let client = plugin
        .state
        .client
        .clone()
        .ok_or(error!("Client must be null at this point"))?;
    let fallback = plugin
        .state
        .fallback
        .clone()
        .ok_or(error!("Fallback must be not null at this point"))?;
    plugin.log(LogLevel::Info, &format!("cln request {request}"));
    let request: BlockByHeight = serde_json::from_value(request)?;
    let mut result: Result<Value, PluginError> = Err(error!("result never init"));
    for client in [client, fallback] {
        result = client.sync_block_by_height(plugin, request.height);
        let Ok(ref result) = result else {
            plugin.log(
                LogLevel::Warn,
                &format!(
                    "client (TODO add the name) return an error: {}",
                    result.clone().err().unwrap()
                ),
            );
            continue;
        };
        break;
    }
    result
}

#[rpc_method(
    rpc_name = "getutxout",
    description = "getutxout to fetch a utx with {txid} and {vout}"
)]
fn getutxout(plugin: &mut Plugin<PluginState>, request: Value) -> Result<Value, PluginError> {
    plugin.log(LogLevel::Debug, "call get utxo");
    let client = plugin
        .state
        .client
        .clone()
        .ok_or(error!("Client must be not null at this point"))?;
    let fallback = plugin
        .state
        .fallback
        .clone()
        .ok_or(error!("Fallback client must be not null at this point"))?;
    plugin.log(LogLevel::Info, &format!("cln request: {request}"));
    let request: GetUTxo = serde_json::from_value(request)?;
    let mut result: Result<Value, PluginError> = Err(error!("result never init"));
    for client in [client, fallback] {
        result = client.sync_get_utxo(plugin, &request.txid, request.vout);
        let Ok(ref result) = result else {
            plugin.log(
                LogLevel::Warn,
                &format!(
                    "client (TODO add the name) return an error: {}",
                    result.clone().err().unwrap()
                ),
            );
            continue;
        };
        break;
    }
    plugin.log(LogLevel::Debug, &format!("{:?}", result));
    result
}

#[rpc_method(
    rpc_name = "sendrawtransaction",
    description = "sendrawtransaction to publish a new transaction"
)]
fn send_rawtransaction(
    plugin: &mut Plugin<PluginState>,
    request: Value,
) -> Result<Value, PluginError> {
    plugin.log(LogLevel::Debug, "call send raw transaction");
    let client = plugin
        .state
        .client
        .clone()
        .ok_or(error!("Client must be not null at this point"))?;
    let fallback = plugin
        .state
        .fallback
        .clone()
        .ok_or(error!("Fallback client must be not null at this point"))?;
    plugin.log(LogLevel::Info, &format!("cln request: {request}"));
    let request: SendRawTx = serde_json::from_value(request)?;

    let mut result: Result<Value, PluginError> = Err(error!("result never init"));
    for client in [client, fallback] {
        result = client.sync_send_raw_transaction(plugin, &request.tx, request.allowhighfees);
        let Ok(ref result) = result else {
            plugin.log(
                LogLevel::Warn,
                &format!(
                    "client (TODO add the name) return an error: {}",
                    result.clone().err().unwrap()
                ),
            );
            continue;
        };
        break;
    }
    result
}
