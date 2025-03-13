
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
Add this flag to allow Remix to connect: --http.corsdomain https://remix.ethereum.org

Test:
curl -X POST http://127.0.0.1:8545 \
     -H "Content-Type: application/json" \
     -H "Origin: https://remix.ethereum.org" \
     --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

{"jsonrpc":"2.0","id":1,"result":"0x2054"}%  
---

Check Reth block number:
curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://localhost:8545
===

npm install -g local-cors-proxy

lcp --proxyUrl http://127.0.0.1:8545
http://localhost:8010/proxy
---
Test sequence
cargo run -p account-seeder -- --account-count 3000 --amount-eth 0.1

cargo run -p reth-scaling --bin tx-generator -- --tx-count 3000 --batch-size 50 --concurrency 50 --target-tps 3000 --use-batching --accounts-file accounts.json

==
Check chain id:
curl -X POST --data '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' http://localhost:8545
Response: {
  "jsonrpc": "2.0",
  "id": 1,
  "result": "0x539"
}