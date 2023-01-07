//! Client dispatch implementation
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use future_common::client::FutureBackend;
use std::boxed::Box;

use crate::plugin::PluginState;
