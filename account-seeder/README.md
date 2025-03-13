cargo run -- --account-count 3000 --amount-eth 0.1

cargo run -- --account-count 5 --amount-eth 0.1
cargo run -- --account-count 5 --rpc-url http://57.128.75.112:8545
cargo run -- --account-count 5 --rpc-url http://57.128.75.112:8545 --amount-eth 10  


cast send --private-key 0xYOUR_PRIVATE_KEY 0xRecipientAddress --value 0.1

cast from-wei $( cast balance ee7eb922ba8a403e73e7db001217fa7bff029579 --rpc-url http://57.128.75.112:8545)

