## Foundry

**Foundry is a blazing fast, portable and modular toolkit for Ethereum application development written in Rust.**

Foundry consists of:

-   **Forge**: Ethereum testing framework (like Truffle, Hardhat and DappTools).
-   **Cast**: Swiss army knife for interacting with EVM smart contracts, sending transactions and getting chain data.
-   **Anvil**: Local Ethereum node, akin to Ganache, Hardhat Network.
-   **Chisel**: Fast, utilitarian, and verbose solidity REPL.

## Documentation

https://book.getfoundry.sh/

## Usage

### Build

```shell
$ forge build
```

### Test

```shell
$ forge test
```

### Format

```shell
$ forge fmt
```

### Gas Snapshots

```shell
$ forge snapshot
```

### Anvil

```shell
$ anvil
```

### Deploy

```shell
$ forge script script/Counter.s.sol:CounterScript --rpc-url <your_rpc_url> --private-key <your_private_key>
```

### Cast

```shell
$ cast <subcommand>
```
cast wallet new
cast send --private-key 0xYourPrefundedAccountPrivateKey 0xYourNewWalletAddress --value 1ether --rpc-url http://127.0.0.1:8545
cast balance 0xYourNewWalletAddress --rpc-url http://127.0.0.1:8545
cast block 0 --rpc-url http://127.0.0.1:8545
cast block 0 --rpc-url http://127.0.0.1:8545 | awk '/miner/ {print $2}'
cast wallet address --private-key
cast rpc --rpc-url http://127.0.0.1:8545 eth_accounts
for addr in 0x80d15b110392ec56a3b44f574d8b457cd6d517bc 0x6b7ce04753de0eb74be389b609686c697e679cc5 0xaa6f4b5e3329a2ec4fcd89c2ceeb7a2231855073 0xcaefdd8494db33c214854e4683fc75e0cb83fa7d 0x0bf2863d29b885af42c4b38040c1d88c99b10c93 0xc2a0bf3af2e9bacf73c9609d790d37b6f544649b 0x40c15306bf2adc9292804e187db778151e248d7a 0x97370c2b7d56bc332d62c6f9f8be634f8d46f46c 0xf95c9d2d81ef2fa6b2260220a3eafa57008a1082 0x53b21f7b0aef1938dabd1b1e5a9688a8d73645fc 0x8a296348da41b577622da35e2c385222a3ecfc34 0xe9bdb3c2de414f4b1f40cf0769c2f3ef586229ed 0x1adb55a01fb76ff81d820615440bddb4cb034708 0x19a046d6c23eab60644c719343f38080262b7ec4 0x3a48e9dd5c7048dd437f9eaff0cca37e2ece4ff1 0x82e083a7fac193b51ff3beb615eb10fc52a1b03b 0x093d80f2c754a969cc800239b90dc02d56348b9b 0xe9a5014137d698b011864950462e1ec2a784eaf9 0x1a4f9f6e95b21e25d45f8812454545337fdf2f0a 0xd9afec0eeb94c5f951ccb978b62dac15e9a591af; do
  cast balance $addr --rpc-url http://127.0.0.1:8545
done

RETH pre-funded accounts:
cast balance 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 --rpc-url http://127.0.0.1:8545

0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 → ~999.7M ETH
0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc → 1M ETH

cast rpc --rpc-url http://127.0.0.1:8545 eth_accounts | grep 0x4a64313b9C0AD6ab3F7C211b985e8154883E0db9
reth account import --private-key <your_private_key>
 cast send 0x4a64313b9C0AD6ab3F7C211b985e8154883E0db9 \
  --value 100ether \
  --rpc-url http://127.0.0.1:8545 \
  --from 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 \
  --private-key 0xss

// Get address from private key
cast wallet address 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
// Convert hex to decimal   
cast --to-dec 0xd3c214ddfefb24034ec0 // 
// Convert wei to ether
cast from-wei 999999499873999999160000

forge inspect SimpleStorage bytecode

Deploy contract:
 cast send --rpc-url http://127.0.0.1:8545  --private-key af1da213daaa4e43086334da042255bdb17a9173e729ea0daae7049bf3411ff7 --create $(forge inspect SimpleStorage bytecode)
To get contract address: (eg 0x182DE6259e21f222a1A867536388aDC519DA50bc)
cast receipt 0xe0dcf3a1dde2f06a81730fab87601c30b38915ed7113a9cf779e3b62c6a06432 \
  --rpc-url http://127.0.0.1:8545

Interact with contract:
cast send 0x182DE6259e21f222a1A867536388aDC519DA50bc "set(uint256)" 42 --rpc-url http://127.0.0.1:8545 --private-key af1da213daaa4e43086334da042255bdb17a9173e729ea0daae7049bf3411ff7

cast call 0x182DE6259e21f222a1A867536388aDC519DA50bc "get()(uint256)"  --rpc-url http://127.0.0.1:8545
forge remove openzeppelin-contracts

forge install OpenZeppelin/openzeppelin-contracts@v5.0.1
forge build --contracts MonetSmartContract.sol

### Help

```shell
$ forge --help
$ anvil --help
$ cast --help
```
