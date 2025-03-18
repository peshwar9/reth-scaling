use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::H160,
};
use std::sync::Arc;
use serde_json::Value;
use std::fs;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Hardcoded inputs
    let rpc_url = "http://34.48.132.251:8845";  // Example RPC URL
    let contract_address = "0xb6db674c6d1861124bb4f6cb23de3ba92eea8347";  // Example contract address

    println!("Verifying contract deployment...");
    println!("RPC URL: {}", rpc_url);
    println!("Contract Address: {}", contract_address);

    // 2. Get deployed bytecode from network
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);
    
    let address: H160 = contract_address.parse()?;
    let deployed_code = client.get_code(address, None).await?;
    
    // 3. Get local compiled bytecode
    let contract_json: Value = serde_json::from_slice(
        include_bytes!("../../../reth-contract/out/MonetSmartContract.sol/MonetSmartContract.json")
    )?;
    
    let local_bytecode = contract_json["deployedBytecode"]["object"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Bytecode not found in contract JSON"))?;

    // 4. Compare bytecodes
    println!("\nDeployed bytecode length: {} bytes", deployed_code.len());
    println!("Local bytecode length: {} bytes", local_bytecode.len() / 2 - 1); // divide by 2 because hex, -1 for '0x' prefix

    // Optional: Print first few bytes of each for comparison
    println!("\nFirst 64 bytes of deployed code: 0x{}", &hex::encode(&deployed_code)[..64]);
    println!("First 64 bytes of local code:    0x{}", &local_bytecode[2..66]);

    if deployed_code == hex::decode(&local_bytecode[2..])? {
        println!("\n✅ Verification successful!");
        println!("The deployed contract matches the local compiled bytecode.");
    } else {
        println!("\n❌ Verification failed!");
        println!("The deployed contract does not match the local compiled bytecode.");
        
        // Optional: Print first mismatch location for debugging
        let deployed_hex = hex::encode(&deployed_code);
        let local_hex = &local_bytecode[2..]; // skip '0x' prefix
        
        for (i, (d, l)) in deployed_hex.chars().zip(local_hex.chars()).enumerate() {
            if d != l {
                println!("First mismatch at position {}: deployed '{}' vs local '{}'", i, d, l);
                break;
            }
        }
    }

    Ok(())
} 