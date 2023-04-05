cln:
    lightningd --testnet --disable-plugin bcli --plugin=/Users/kodylow/Documents/github/satoshi/target/debug/satoshi_plugin --log-level=debug

kill-cln:
    pgrep -f "lightningd" | xargs -r kill -9
