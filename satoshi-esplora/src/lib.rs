use clightningrpc_plugin::errors::PluginError;
use esplora_client::{BlockingClient, Builder};
use satoshi_common::client::SatoshiBackend;

enum Network {
    Bitcoin(String),
    Testnet(String),
    Liquid(String),
    BitcoinTor(String),
    TestnetTor(String),
    LiquidTor(String),
}

impl Network {
    pub fn url(&self) -> String {
        match &self {
            Self::Bitcoin(url) => url.to_string(),
            Self::Liquid(url) => url.to_string(),
            Self::Testnet(url) => url.to_string(),
            Self::BitcoinTor(url) => url.to_string(),
            Self::TestnetTor(url) => url.to_string(),
            Self::LiquidTor(url) => url.to_string(),
        }
    }
}

impl TryFrom<&str> for Network {
    type Error = PluginError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "bitcoin" => Ok(Self::Bitcoin("https://blockstream.info".to_owned())),
            "bitcoin/tor" => Ok(Self::BitcoinTor(
                "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion".to_owned(),
            )),
            "testnet" => Ok(Self::Testnet("https://blockstream.info/testnet".to_owned())),
            "testnet/tor" => Ok(Self::TestnetTor(
                "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion/testnet"
                    .to_owned(),
            )),
            _ => Err(PluginError::new(
                -1,
                &format!("network {value} not supported"),
                None,
            )),
        }
    }
}

pub struct Esplora {
    network: Network,
    client: BlockingClient,
}

impl Esplora {
    pub fn new(network: &str) -> Result<Self, PluginError> {
        let network = Network::try_from(network)?;
        let builder = Builder::new(&network.url());
        Ok(Self {
            network,
            client: builder.build_blocking().unwrap(),
        })
    }
}

impl<T: Clone> SatoshiBackend<T> for Esplora {
    type Error = PluginError;

    fn sync_block_by_height(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        height: u64,
    ) -> Result<serde_json::Value, Self::Error> {
        todo!()
    }

    fn sync_chain_info(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, Self::Error> {
        todo!()
    }

    fn sync_estimate_fees(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
    ) -> Result<serde_json::Value, Self::Error> {
        todo!()
    }

    fn sync_get_utxo(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        _: &str,
        _: u64,
    ) -> Result<serde_json::Value, Self::Error> {
        todo!()
    }

    fn sync_send_raw_transaction(
        &self,
        _: &mut clightningrpc_plugin::plugin::Plugin<T>,
        _: &str,
        _: bool,
    ) -> Result<serde_json::Value, Self::Error> {
        todo!()
    }
}
