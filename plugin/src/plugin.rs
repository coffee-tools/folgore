//! Plugin definition.
use clightningrpc_plugin::{
    commands::RPCCommand, errors::PluginError, plugin::Plugin, types::LogLevel,
};

#[derive(Clone)]
pub struct PluginState {}

impl PluginState {
    fn new() -> Self {
        PluginState {}
    }
}

pub fn build_plugin() -> Plugin<PluginState> {
    todo!()
}
