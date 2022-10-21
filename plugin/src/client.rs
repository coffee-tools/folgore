//! Client dispach implementation
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use future_common::client::FutureBackend;
use std::boxed::Box;

pub struct PluginBuilder;

impl PluginBuilder {
    /// Build client for the plugin.
    pub fn build<T: Clone>(_: &Plugin<T>) -> Box<dyn FutureBackend<T, Error = PluginError>> {
        todo!()
    }
}
