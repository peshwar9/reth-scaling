

export ETH_RPC_URL=

// Check balances
cargo run --bin seed balances --num-accounts 5

// Prepare accounts for nodes
cargo run --bin seed prepare --num-accounts 5 --num-nodes 3

// Fund accounts
cargo run --bin seed fund --num-nodes 3 --num-accounts 5 --amount-wei 1
Min needed (0.0001 ETH):
cargo run --bin seed fund --num-nodes 2 --num-accounts 2 --amount-wei 100000000000000


// Fund individual node sender accounts
cargo run --bin seed fund-node --node 2 --amount-eth 0.000050

// Check balances of individual node sender accounts
cargo run --bin seed node-balances --node 2

// Defund accounts
cargo run --bin seed defund --num-nodes 3 --num-accounts 5

// Send eth cross-chain
cargo run --bin seed send-eth --num-nodes 2 --num-accounts 2 --amount-wei 1


