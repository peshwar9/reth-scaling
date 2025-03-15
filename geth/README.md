#### To run geth
geth --datadir ~/.ethereum \
  --networkid 9013 \
  --syncmode full \
  --http --http.addr 0.0.0.0 --http.port 8545 \
  --http.api "eth,net,web3,debug,txpool,personal" --http.corsdomain "57.128.124.17" \
  --mine --miner.etherbase 0x0a985c64188b22af9f21b78451DcB6dC78435B2e \
  --unlock 0x0a985c64188b22af9f21b78451DcB6dC78435B2e --password password.txt \
  --allow-insecure-unlock \
  --nodiscover