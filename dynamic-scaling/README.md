

export ETH_RPC_URL=

// Prepare accounts for nodes
cargo run --bin seed prepare --num-accounts 5 --num-nodes 3


// Fund individual node sender accounts
cargo run --bin seed fund-node --node 2 --amount-eth 0.000050

// Check balances of individual node sender accounts
cargo run --bin seed node-balances --node 2

// Defund Node
cargo run --bin seed defund-node --node 1

// Send eth cross-chain
cargo run --bin seed send-eth --num-nodes 2 --num-accounts 2 --amount-wei 1



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
