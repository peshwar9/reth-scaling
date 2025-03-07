
cargo build

# Run a specific project
cargo run -p account-seeder
cargo run -p reth-scaling --bin tx-generator

---

# Add the tap and install reth
brew tap paradigmxyz/brew
brew install reth

# Start RETH with your genesis file:

rm -rf ~/Library/Application\ Support/reth/*/db

---
reth init --datadir=./reth-dev-data --chain=dev


These work:
reth node --chain dev --http --http.port 8545 --dev --dev.block-time 10
reth node --chain dev --http --http.port 8545 --dev --dev.block-time 1s --rpc.max-connections 3000  --builder.gaslimit 60000000
reth node --chain dev --http --http.addr 0.0.0.0 --http.port 8545 --dev --dev.block-time 1s --rpc.max-connections 3000 --builder.gaslimit 60000000 --txpool.gas-limit 60000000 --http.api "eth,net,web3,debug,trace,txpool" --rpc.max-request-size 30 --rpc.max-response-size 200 --txpool.max-tx-input-bytes 262144
---

Check Reth block number:
curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:8545
===


Test sequence
cargo run -p account-seeder -- --account-count 3000 --amount-eth 0.1

cargo run -p reth-scaling --bin tx-generator -- --tx-count 3000 --batch-size 50 --concurrency 50 --target-tps 3000 --use-batching --accounts-file accounts.json

