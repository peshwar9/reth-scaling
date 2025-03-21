

export ETH_RPC_URL=

# Prepare accounts for nodes
cargo run --bin seed prepare --num-accounts 5 --num-nodes 3

# prepare new
cargo run --bin seed -- prepare-new --node 1 --num-accounts 10

# Fund individual node sender accounts
cargo run --bin seed fund-node --node 2 --amount-eth 0.000050

# Check balances of individual node sender accounts
cargo run --bin seed node-balances --node 2

# Defund Node
cargo run --bin seed defund-node --node 1

# Send eth cross-chain one-way
// This will send 3 accounts from node 1 to node 2, 2 times
cargo run --bin seed -- send-eth-1way --from-node 1 --to-node 2 --num-accounts 3 --amount-wei 1 --rounds 2

Log format: status,block_number,round,tx_hash,from_chain,to_chain,from_addr,to_addr,amount


# Send eth cross-chain multi-way

cargo run --bin seed -- send-eth-nway --num-nodes 3 --num-accounts 2 --amount-wei 1 --rounds 2

# Run indefinitely
cargo run --bin seed send-eth-loop --num-nodes 2 --num-accounts 2 --amount-wei 1 --rounds "#"

# Send eth burst:
## For zero gas price:
cargo run --bin seed -- send-eth-burst --from-node 1 --to-node 2 --num-txs 10 --amount-wei 1 --zero-gas-price

## For non-zero gas price (omit the flag):
cargo run --bin seed -- send-eth-burst --from-node 1 --to-node 2 --num-txs 10 --amount-wei 1

Sequence of steps:
1. Prepare accounts
2. Fund Node
3. Check balances
4. Send eth cross-chain
5. Defund Node

The .env file should contain the following variables:

ETH_RPC_URL=http://34.21.80.98:8845
MASTER_WALLET_KEY=0xxxxxxxxxx
MASTER_WALLET_ADDRESS=0x64dd863d6b65486b4d15a483c9a9b382bbb609f8

NODE1_CHAINID=9012
NODE1_CONTRACT=0x9a3f2c925021d158f968070295c4f3d67af596cd
NODE1_RPC=http://34.21.80.98:8845
NODE2_CHAINID=9013
NODE2_CONTRACT=0x9a3f2c925021d158f968070295c4f3d67af596cd
NODE2_RPC=http://34.48.132.251:8845
NODE3_CHAINID=9014
NODE3_CONTRACT=0x9a3f2c925021d158f968070295c4f3d67af596cd
NODE3_RPC=http://34.48.205.25:8845


# Proof verifier

# Run with transaction hash
RUST_LOG=debug cargo run --bin proof_verifier 0x06f3a614a727072fa42230a9fe6096a469840707badf020321691519bb02cf8f