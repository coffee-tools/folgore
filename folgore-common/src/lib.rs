pub mod client;
pub mod stragegy;

pub mod utils {
    pub use bitcoin_hashes;

    #[macro_export]
    macro_rules! hex (($hex:expr) => (<Vec<u8> as bitcoin_hashes::hex::FromHex>::from_hex($hex).unwrap()));
    pub use hex;

    pub struct ByteBuf<'a>(pub &'a [u8]);

    impl<'a> std::fmt::LowerHex for ByteBuf<'a> {
        fn fmt(&self, fmtr: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
            for byte in self.0 {
                fmtr.write_fmt(format_args!("{:02x}", byte))?;
            }
            Ok(())
        }
    }
}

pub mod prelude {
    // FIXME: remove the followung clightninrpc from
    // here
    pub use clightningrpc_common as cln;
    pub use clightningrpc_plugin as cln_plugin;
    pub use log;
    pub use serde_json as json;
}

pub mod cln {
    pub use clightningrpc as rpc;
    pub use clightningrpc_common::*;
    pub use clightningrpc_plugin as plugin;
}

pub type Result<T> = std::result::Result<T, clightningrpc_plugin::errors::PluginError>;
