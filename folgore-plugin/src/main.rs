#![deny(clippy::unwrap_used)]
mod model;
mod plugin;
mod recovery;

fn main() {
    let plugin = plugin::build_plugin();
    plugin.start();
}

#[cfg(test)]
use std::sync::Once;

#[cfg(test)]
static INIT: Once = Once::new();

#[cfg(test)]
fn configure_tests() {
    INIT.call_once(|| {
        env_logger::init();
    });
}
