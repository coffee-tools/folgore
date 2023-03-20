//! Client dispatch implementation
use clightningrpc_plugin::errors::PluginError;
use clightningrpc_plugin::plugin::Plugin;
use satoshi_common::client::FutureBackend;
use std::boxed::Box;

use crate::plugin::PluginState;
