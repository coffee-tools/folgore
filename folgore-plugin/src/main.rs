#![deny(clippy::unwrap_used)]
mod model;
mod plugin;
mod recovery;

use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    let plugin = plugin::build_plugin();
    plugin.start().await;
    Ok(())
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
