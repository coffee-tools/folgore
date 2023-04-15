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
cp ./target/debug/satoshi_plugin /home/<user>/.lightning/plugin
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
- `bitcoin-esplora-url`: The URL of the esplora server, by default using the Blockstream API

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
