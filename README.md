<div align="center">
  <h1>Folgore</h1>

  <p>
    <strong> Universal Bitcoin Backend for Core lightning </strong>
  </p>

  <h4>
    <a href="https://github.com/coffee-tools/folgore">Home Page</a>
  </h4>
</div>

Universal Bitcoin backend for core lightning with BIP 157 support

## Name History

The name of this plugin was choose by the community on [twitter](https://twitter.com/PalazzoVincenzo/status/1643703009082236933?s=20), and
we choose the name `folgore` that means `lightning` in Italian ([see](https://dictionary.cambridge.org/us/dictionary/italian-english/folgore)).
This idea was pretty cool because pin a the period where this plugin is born, in particular Italian Governament
ban English words in some context, so I was thinking that this name was pretty cool for a plugin and also
to joke around this Italian choice.

## How to Install

To install the plugin in core lightning you need to have the [rust
installed](https://www.rust-lang.org/tools/install)

### Install the plugin Manually

```
>> make
cp ./target/debug/folgore_plugin /home/<user>/.lightning/plugin
```

### With a plugin manager

This plugin will support [coffee](https://coffee-docs.netlify.app/introduction.html) as plugin manager, and it is also the 
raccomended way to install a this plugin in core lightning.

To install it you need just to [install coffee](https://coffee-docs.netlify.app/install-coffee.html) and then
run the following commands

```bash
>> coffee --network <your network> add remote folgore-git https://github.com/coffee-tools/folgore.git
>> coffee --network <your network> install folgore
```

## How to configure

The plugin is developed to work out of the box without any extra configuration, but if you want 
make a customization, these are some configuration options:

- `bitcoin-client`: The client name that the plugin need to use, by default `esplora` but the following clients are supported:
   - `nakamoto`: Bitcoin node implementation with the BIP 157 support;
   - `esplora`: Rest API to support esplora like backend,
   - `bitcoind`: Bitcoin Core implementation
- `bitcoin-esplora-url`: The URL of the esplora server, by default using the Blockstream API
- `bitcoin-rpcurl`: The URL of bitcoin core (for now it support http only and not https)
- `bitcoin-rpcuser`: Bitcoin core RPC user inside for authentication;
- `bitcoin-rpcpassword`: Bitcoin core RPC password for authentication.
- `bitcoin-fallback-client`: Bitcoin fallback client, in the case one of the client fails, the plugin use another backend for the request.

## BIP 157 support

This plugin allow the support of the BIP 157 [Client Side Block Filtering](https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki) in core lightning
in a alpha mode.

In fact, the plugin is build on top of [nakamoto](https://github.com/cloudhead/nakamoto) with a bunch of fixes that are 
proposed as PR in the main repository, in addition the current version of the plugin is build on top of an 
core lightning RFC PR [#6181](https://github.com/ElementsProject/lightning/pull/6181), so the support the the 
BIP 157 is very experimental and there is still work to do.

However, it is the perfect time to stress test the plugin and report not known bugs, so please
if you want test the BIP 157 support consider to install the plugin with one of the 
previous method, and then run `lightnind` with the option `bitcoin-client=nakamoto` and then
report any bugs that you find with an [issue](https://github.com/coffee-tools/folgore/issues).

In addition, if you are running the plugin with a not clean node, you should run nakamoto and wait 
that it will sync the initial information for the chain otherwise core lightning will crash because 
nakamoto return an older block height (this issue is fixed with the PR #6181 on core lightning).

To run nakamoto you can run the following command 

``` bash
git clone https://github.com/vincenzopalazzo/nakamoto.git && cd nakamoto
git checkout macros/client_model-fixes
cd node
cargo run -- --log debug --testnet
```

An than lets wait a while that nakamoto will sync with the network.

## License

<div align="center">
  <img src="https://opensource.org/files/osi_keyhole_300X300_90ppi_0.png" width="150" height="150"/>
</div>

 Universal Bitcoin backend for core lightning with BIP 157 support

 Copyright (C) 2020-2021 Vincenzo Palazzo vincenzopalazzodev@gmail.com
 
 This program is free software; you can redistribute it and/or modify
 it under the terms of the GNU General Public License as published by
 the Free Software Foundation; either version 2 of the License.
 
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU General Public License for more details.
 
 You should have received a copy of the GNU General Public License along
 with this program; if not, write to the Free Software Foundation, Inc.,
 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
