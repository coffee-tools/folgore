//! Plugin definition.
use clightningrpc_plugin::{
    commands::RPCCommand, errors::PluginError, plugin::Plugin, types::LogLevel,
};
use future_common::client::FutureBackend;

#[derive(Clone)]
pub struct PluginState<'tcx> {
    client: &'tcx Option<Box<dyn FutureBackend<PluginState<'tcx>, Error = PluginError>>>,
}

impl PluginState<'_> {
    fn new() -> Self {
        PluginState { client: &None }
    }
}

pub fn build_plugin<'c>() -> Plugin<PluginState<'c>> {
    todo!()
}
