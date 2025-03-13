cargo build
cargo run -- --gen-accounts --gen-genesis
cargo run -- --tx-count 10000 --batch-size 100 --target-tps 3000 --use-batching --accounts-file accounts.json




# check balance

curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x3a50d60a94c7ce6d6fd9046a077179df19207912", "latest"],"id":1}' http://localhost:8545

cargo run --bin tx-generator -- --tx-count 3000 --batch-size 50 --concurrency 50 --target-tps 3000 --use-batching --accounts-file accounts.json
