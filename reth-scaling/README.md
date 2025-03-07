cargo build
cargo run -- --gen-accounts --gen-genesis
cargo run -- --tx-count 10000 --batch-size 100 --target-tps 3000 --use-batching --accounts-file accounts.json


Reth setup:

# Download RETH (if you're on a Mac)
curl -L https://github.com/paradigmxyz/reth/releases/latest/download/reth-x86_64-apple-darwin.tar.gz -o reth.tar.gz
tar -xzf reth.tar.gz
chmod +x reth

OR:
# Add the tap and install reth
brew tap paradigmxyz/brew
brew install reth

# Start RETH with your genesis file:

reth node --config high-throughput.toml

reth node --http --http.port 8545 --http.addr 127.0.0.1 --config high-throughput.toml

rm -rf ~/Library/Application\ Support/reth/*/db


reth node --chain=dev --datadir=./reth-bench-data --genesis=simple-genesis.json --http --http.addr=127.0.0.1 --http.port=8545

# check balance

curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x3a50d60a94c7ce6d6fd9046a077179df19207912", "latest"],"id":1}' http://localhost:8545
