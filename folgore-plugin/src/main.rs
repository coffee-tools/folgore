#![deny(clippy::unwrap_used)]
mod client;
mod model;
mod plugin;
mod recovery;

fn main() {
    let plugin = plugin::build_plugin();
    plugin.start();
}
