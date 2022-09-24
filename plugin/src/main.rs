mod plugin;

fn main() {
    let mut plugin = plugin::build_plugin();
    plugin.start();
}
