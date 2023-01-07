//! Plugin definition.
use clightningrpc_plugin::commands::RPCCommand;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use clightningrpc_plugin::types::LogLevel;
use future_common::client::FutureBackend;
use nakamoto_cln_client::Nakamoto;
use serde_json::{json, Value};

pub struct PluginState<'tcx> {
    client: Option<Box<dyn FutureBackend<PluginState<'tcx>, Error = PluginError>>>,
}

impl PluginState<'_> {
    fn new() -> Self {
        PluginState { client: None }
    }
}

pub fn build_plugin<'c>() -> Plugin<PluginState<'c>> {
    let plugin = Plugin::new(PluginState::new(), false)
        .on_init(&on_init)
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
        .to_owned();
    plugin
}

pub fn on_init(plugin: &mut Plugin<PluginState<'_>>) -> Value {
    let dir = plugin.configuration.clone().unwrap().lightning_dir.as_str();
    let client: String = plugin.get_opt("satoshi-client").unwrap();
    match client.as_str() {
        "satoshi-client" => {
            let nakamoto = Nakamoto::new().unwrap();
            plugin.state.client = Some(Box::new(nakamoto));
        }
        _ => {
            return json!({
                "disable": "client not supported"
            })
        }
    }
    json!({})
}

// FIXME use the plugin_macros to semplify all this code
#[derive(Clone)]
struct GetChainInfoRPC {}

impl RPCCommand<PluginState<'_>> for GetChainInfoRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        _request: &'c Value,
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
        _request: &'c Value,
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
        _request: &'c Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        // FIXME: analyze the response to get the height
        client.sync_block_by_height(plugin, 0)
    }
}

#[derive(Clone)]
struct GetUtxOutRPC {}

impl RPCCommand<PluginState<'_>> for GetUtxOutRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        _request: &'c Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        // FIXME: analyze the response to get the input
        client.sync_get_utxo(plugin)
    }
}

#[derive(Clone)]
struct SendRawTransactionRPC {}

impl RPCCommand<PluginState<'_>> for SendRawTransactionRPC {
    fn call<'c>(
        &self,
        plugin: &mut Plugin<PluginState<'_>>,
        _request: &'c Value,
    ) -> Result<Value, PluginError> {
        plugin.log(LogLevel::Debug, "call get chain info");
        let mut plg = plugin.to_owned();
        let client = plg.state.client.as_mut().unwrap();
        // FIXME: analyze the response to get the input
        client.sync_send_raw_transaction(plugin, "", true)
    }
}

impl<'tcx> Clone for PluginState<'tcx> {
    fn clone(&self) -> Self {
        self.to_owned()
    }
}
