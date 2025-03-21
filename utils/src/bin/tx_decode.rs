use ethers::{
    prelude::*,
    providers::{Http, Provider},
    types::{Transaction, TransactionReceipt, H256, U256},
    abi::{Abi, Event, RawLog, ParamType, Token},
};
use eyre::Result;
use std::sync::Arc;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::str::FromStr;  // Added for H256::from_str
use std::env;
use dotenv::dotenv;
use hex;

#[tokio::main]
async fn main() -> Result<()> {

    dotenv().ok();
    
    // Set up the provider using NODE1_RPC from env
    let rpc_url = env::var("NODE4_RPC")
        .expect("RPC_URL must be set in .env file");

    let tx_hash = "0x664d2ad29902f3cf5218be09d3caf2c1808c9fc387344ce21f077eb932b0f6ef";

    // Create provider
    let provider = Provider::<Http>::try_from(rpc_url.clone())?;
    let client = Arc::new(provider);

    // Get transaction hash
    let tx_hash = tx_hash.parse::<H256>()?;

    println!("Fetching transaction details for: {:#x}", tx_hash);
    println!("RPC URL: {}", rpc_url);

    // Get transaction
    let tx = client.get_transaction(tx_hash).await?
        .ok_or_else(|| eyre::eyre!("Transaction not found"))?;

    // Get transaction receipt
    let receipt = client.get_transaction_receipt(tx_hash).await?
        .ok_or_else(|| eyre::eyre!("Transaction receipt not found"))?;

    // Load contract ABI
    let abi_path = Path::new("src/bin/abi.json");
    let abi_content = fs::read_to_string(abi_path)?;
    let abi_json: Value = serde_json::from_str(&abi_content)?;
    let abi: ethers::abi::Abi = serde_json::from_value(abi_json["abi"].clone())?;

    println!("\nTransaction Details:");
    println!("------------------");
    print_transaction_details(&tx, &receipt)?;

    println!("\nFunction Call:");
    println!("--------------");
    if let Some(input) = decode_function_call(&tx.input, &abi) {
        println!("{}", input);
    } else {
        println!("Could not decode function call");
    }

    println!("\nEvents:");
    println!("-------");
    decode_logs(&receipt.logs, &abi, &tx)?;

    Ok(())
}

fn print_transaction_details(tx: &Transaction, receipt: &TransactionReceipt) -> Result<()> {
    let status = receipt.status.unwrap().as_u64();
    println!("Status: {}", if status == 1 { "Success" } else { "Failed" });
    
    // If transaction failed, try multiple ways to get the revert reason
    if status == 0 {
        println!("Failure Reason:");
        
        // 1. Check standard error logs
        if let Some(first_log) = receipt.logs.first() {
            let error_sig = H256::from_str("0x08c379a0").expect("Invalid error signature");
            if first_log.topics.len() > 0 && first_log.topics[0] == error_sig {
                if first_log.data.len() > 68 {
                    let error_msg = String::from_utf8_lossy(&first_log.data[68..]);
                    println!("  From logs: {}", error_msg);
                }
            }
        }

        // 2. Check for panic error (0x4e487b71)
        for log in &receipt.logs {
            let panic_sig = H256::from_str("0x4e487b71").expect("Invalid panic signature");
            if log.topics.len() > 0 && log.topics[0] == panic_sig {
                println!("  Panic detected: {:?}", log.data);
            }
        }

        // 3. Check if all gas was used (likely ran out of gas)
        if receipt.gas_used.unwrap() >= tx.gas {
            println!("  Likely out of gas - used entire gas limit of {}", tx.gas);
        }

        // 4. Try to decode revert data from transaction
        if let Some(data) = &receipt.logs.first().map(|log| &log.data) {
            println!("  Raw revert data: 0x{}", hex::encode(data));
        } else {
            println!("  No error data available");
        }
    }

    println!("Block: #{}", receipt.block_number.unwrap());
    println!("From: {:#x}", tx.from);
    if let Some(to) = tx.to {
        println!("To: {:#x}", to);
    }
    println!("Value: {} wei", tx.value);
    
    // Handle gas values
    let gas_limit = tx.gas;  // This is already U256
    println!("Gas Limit: {}", gas_limit);
    println!("Gas Used: {}", receipt.gas_used.unwrap());
    println!("Gas Price: {} wei", tx.gas_price.unwrap());
    
    // Print gas usage percentage
    let gas_used = receipt.gas_used.unwrap();
    let usage_percent = (gas_used.as_u64() as f64 / gas_limit.as_u64() as f64) * 100.0;
    println!("Gas Usage: {:.1}%", usage_percent);
    
    println!("Nonce: {}", tx.nonce);

    Ok(())
}

fn decode_function_call(input: &Bytes, abi: &ethers::abi::Abi) -> Option<String> {
    if input.len() < 4 {
        return None;
    }

    // Get function selector (first 4 bytes)
    let selector = &input[0..4];

    // Find matching function
    for function in abi.functions() {
        if function.short_signature() == selector {
            // Try to decode parameters
            match function.decode_input(&input[4..]) {
                Ok(tokens) => {
                    let params: Vec<String> = tokens.iter()
                        .zip(function.inputs.iter())
                        .map(|(token, param)| {
                            match token {
                                Token::Uint(val) if param.name == "chainID" => {
                                    // Convert U256 to bytes
                                    let mut bytes = [0u8; 32];
                                    val.to_big_endian(&mut bytes);
                                    let le_val = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
                                    let be_val = u32::from_be_bytes(bytes[28..32].try_into().unwrap());
                                    format!("{}: {} (BE: {}, LE: {}, raw: {:?})", 
                                        param.name, val, be_val, le_val, &bytes[28..32])
                                },
                                _ => format!("{}: {}", param.name, token)
                            }
                        })
                        .collect();
                    
                    return Some(format!("{}({})", 
                        function.name,
                        params.join(", ")
                    ));
                }
                Err(_) => return Some(format!("{}(...)", function.name)),
            }
        }
    }

    None
}

fn decode_logs(logs: &[Log], abi: &ethers::abi::Abi, tx: &Transaction) -> Result<()> {
    for log in logs {
        println!("\nLog from contract at {:#x}:", log.address);
        
        // Check if log address matches transaction destination
        if log.address != tx.to.unwrap() {
            println!("⚠️  Warning: Event emitted from different contract than transaction destination");
            println!("    Transaction sent to: {:#x}", tx.to.unwrap());
            println!("    Event emitted from: {:#x}", log.address);
        }

        for event in abi.events() {
            let event_sig = event.signature();
            if log.topics[0] == event_sig {
                println!("Event: {}", event.name);
                
                let raw_log = RawLog {
                    topics: log.topics.clone(),
                    data: log.data.to_vec(),
                };

                match event.parse_log(raw_log) {
                    Ok(parsed) => {
                        println!("  Contract Address: {:#x}", log.address);
                        println!("  Block Number: {}", log.block_number.unwrap_or_default());
                        println!("  Transaction Index: {}", log.transaction_index.unwrap_or_default());
                        println!("  Log Index: {}", log.log_index.unwrap_or_default());
                        println!("  Parameters:");
                        
                        for (param, value) in event.inputs.iter().zip(parsed.params) {
                            let formatted_value = match &value.value {
                                Token::Uint(val) => {
                                    match param.kind {
                                        ParamType::Uint(32) => format!("{}", val.as_u32()),
                                        ParamType::Uint(256) => format!("{}", val),
                                        _ => val.to_string(),
                                    }
                                },
                                Token::Address(addr) => format!("{:#x}", addr),
                                _ => value.value.to_string(),
                            };
                            println!("    {}: {}", param.name, formatted_value);
                        }
                    }
                    Err(e) => {
                        println!("  Failed to parse log: {}", e);
                    }
                }
            }
        }
    }
    Ok(())
} 