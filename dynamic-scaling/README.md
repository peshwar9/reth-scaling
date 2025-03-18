

export ETH_RPC_URL=

// Prepare accounts for nodes
cargo run --bin seed prepare --num-accounts 5 --num-nodes 3


// Fund individual node sender accounts
cargo run --bin seed fund-node --node 2 --amount-eth 0.000050

// Check balances of individual node sender accounts
cargo run --bin seed node-balances --node 2

// Defund Node
cargo run --bin seed defund-node --node <node_number>

// Send eth cross-chain
cargo run --bin seed send-eth --num-nodes 2 --num-accounts 2 --amount-wei 1


