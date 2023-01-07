//! Plugin definition.
use clightningrpc_plugin::commands::RPCCommand;
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use clightningrpc_plugin::types::LogLevel;
use future_common::client::FutureBackend;

pub struct PluginState<'tcx> {
    client: Option<Box<dyn FutureBackend<PluginState<'tcx>, Error = PluginError>>>,
}

impl PluginState<'_> {
    fn new() -> Self {
        PluginState { client: None }
    }
}

pub fn build_plugin<'c>() -> Plugin<PluginState<'c>> {
    let plugin = Plugin::new(PluginState::new(), false);
    plugin
}

impl<'tcx> Clone for PluginState<'tcx> {
    fn clone(&self) -> Self {
        self.to_owned()
    }
}
