#![feature(async_fn_in_trait)]
#![feature(associated_type_defaults)]
#![allow(incomplete_features)]
mod client;
mod model;
mod plugin;

fn main() {
    let plugin = plugin::build_plugin();
    plugin.start();
}
