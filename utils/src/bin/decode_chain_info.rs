use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::{Address, U256},
};
use std::env;
use dotenv::dotenv;
use eyre::Result;
use std::sync::Arc;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    
    // Get RPC and contract address from env
    let rpc_url = env::var("NODE2_RPC")
        .expect("RPC must be set in .env file");
    let contract_addr = env::var("NODE2_CONTRACT")
        .expect("CONTRACT must be set in .env file")
        .parse::<Address>()?;

    // Create provider
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);

    // Load contract ABI
    let contract_json: Value = serde_json::from_str(include_str!("../../../reth-contract/out/MonetSmartContract.sol/MonetSmartContract.json"))?;
    let abi: ethers::abi::Abi = serde_json::from_value(contract_json["abi"].clone())?;
    
    // Create contract instance
    let contract = Contract::new(contract_addr, abi, client);

    // Call getDestinationChainInfo
    let chain_id: u32 = 9012;
    let result: (String, Address, Vec<U256>) = contract
        .method("getDestinationChainInfo", chain_id)?
        .call()
        .await?;

    // Decode and print results
    println!("Chain ID: {}", chain_id);
    println!("RPC URL: {}", result.0);
    println!("Contract Address: {:?}", result.1);
    println!("Additional Info:");
    for (i, value) in result.2.iter().enumerate() {
        println!("  Value {}: {}", i, value);
    }

    Ok(())
} 