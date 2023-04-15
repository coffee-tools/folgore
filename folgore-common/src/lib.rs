pub mod client;

pub mod utils {
    pub use bitcoin_hashes;

    #[macro_export]
    macro_rules! hex (($hex:expr) => (<Vec<u8> as bitcoin_hashes::hex::FromHex>::from_hex($hex).unwrap()));
    pub use hex;
}
