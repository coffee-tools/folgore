#![feature(async_fn_in_trait)]
#![feature(associated_type_defaults)]
#![allow(incomplete_features)]
mod plugin;

fn main() {
    let mut plugin = plugin::build_plugin();
    plugin.start();
}
