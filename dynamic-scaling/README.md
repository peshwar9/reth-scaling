

export ETH_RPC_URL=

// Check balances
cargo run --bin seed balances --num-accounts 5

// Prepare accounts for nodes
cargo run --bin seed prepare --num-accounts 5 --num-nodes 3

// Fund accounts
cargo run --bin seed fund --num-nodes 3 --num-accounts 5 --amount-wei 1

// Defund accounts
cargo run --bin seed defund --num-nodes 3 --num-accounts 5

// Send eth cross-chain
cargo run --bin seed send-eth --num-nodes 3 --num-accounts 5 --amount-wei 1
