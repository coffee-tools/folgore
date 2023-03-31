//! Plugin definition.
use satoshi_nakamoto::Nakamoto;
use serde_json::{json, Value};

use clightningrpc_common::types::Request;
use clightningrpc_plugin::commands::RPCCommand;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use clightningrpc_plugin::types::LogLevel;

use satoshi_common::client::SatoshiBackend;
use satoshi_nakamoto::Config;

use crate::model::{BlockByHeight, GetUTxo, SendRawTx};

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

pub struct PluginState<'tcx> {
    pub(crate) client: Option<Box<dyn SatoshiBackend<PluginState<'tcx>, Error = PluginError>>>,
}

impl PluginState<'_> {
    fn new() -> Self {
        PluginState { client: None }
    }

    fn new_client(&mut self, client: &str) -> Result<(), PluginError> {
        let client = ClientType::try_from(client)?;
        match client {
            ClientType::Nakamoto => {
                // FIXME: make a proper configuration
                let config = Config::default();
                let client = Nakamoto::new(config)
                    .map_err(|err| PluginError::new(-1, &format!("{err}"), None))?;
                self.client = Some(Box::new(client));
                Ok(())
            }
            ClientType::Esplora => {
                todo!()
            }
        }
    }
}

pub fn build_plugin<'c>() -> Plugin<PluginState<'c>> {
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
            "satoshi-client",
            "string",
            Some("nakamoto".to_owned()),
            "Set up the client to use",
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
        .on_init(&on_init)
        .to_owned();
    plugin
}

fn on_init(plugin: &mut Plugin<PluginState>) -> Value {
    let client: String = plugin.get_opt("satoshi-client").unwrap();
    if let Err(err) = plugin.state.new_client(&client) {
        plugin.log(LogLevel::Debug, &format!("{err}"));
    };
    json!({})
}

// FIXME use the plugin_macros to semplify all this code
#[derive(Clone)]
struct GetChainInfoRPC {}

impl RPCCommand<PluginState<'_>> for GetChainInfoRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        _: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        client.sync_chain_info(plugin)
    }
}

#[derive(Clone)]
struct EstimateFeesRPC {}

impl RPCCommand<PluginState<'_>> for EstimateFeesRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        _: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        client.sync_estimate_fees(plugin)
    }
}

#[derive(Clone)]
struct GetRawBlockByHeightRPC {}

impl RPCCommand<PluginState<'_>> for GetRawBlockByHeightRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        let request: Request<BlockByHeight> = serde_json::from_value(request)?;
        client.sync_block_by_height(plugin, request.params.height)
    }
}

#[derive(Clone)]
struct GetUtxOutRPC {}

impl RPCCommand<PluginState<'_>> for GetUtxOutRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        let request: Request<GetUTxo> = serde_json::from_value(request)?;
        client.sync_get_utxo(plugin, &request.params.txid, request.params.vout)
    }
}

#[derive(Clone)]
struct SendRawTransactionRPC {}

impl RPCCommand<PluginState<'_>> for SendRawTransactionRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        request: Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        let request: Request<SendRawTx> = serde_json::from_value(request)?;
        client.sync_send_raw_transaction(plugin, &request.params.tx, request.params.allowhighfees)
    }
}

impl<'tcx> Clone for PluginState<'tcx> {
    fn clone(&self) -> Self {
        self.to_owned()
    }
}
