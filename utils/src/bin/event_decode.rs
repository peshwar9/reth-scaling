use ethers::abi::{Abi, Event, EventParam, ParamType, RawLog, Token, decode};
use ethers::types::{H256, Log, U256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use hex;

#[derive(Debug, Deserialize, Serialize)]
struct Receipt {
    logs: Vec<LogEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LogEntry {
    address: String,
    topics: Vec<H256>,
    data: String,
}

fn main() {
    // Load receipt.txt from src/bin directory
    let receipt_path = Path::new("src/bin/receipt.txt");
    let receipt_content = fs::read_to_string(receipt_path)
        .expect("Failed to read receipt file");
    
    // Extract logs line
    let logs_json = receipt_content.lines()
        .find(|line| line.starts_with("logs"))
        .expect("Logs field not found")
        .split_once("[")
        .map(|(_, logs)| format!("[{}", logs))
        .expect("Invalid logs format");
    
    let receipt_logs: Vec<LogEntry> = serde_json::from_str(&logs_json)
        .expect("Failed to parse logs JSON");
    
    // Load abi.json from src/bin directory
    let abi_path = Path::new("src/bin/abi.json");
    let abi_content = fs::read_to_string(abi_path)
        .expect("Failed to read ABI file");
    let abi: Value = serde_json::from_str(&abi_content)
        .expect("Failed to parse ABI JSON");
    
    let events: Vec<&Value> = abi["abi"].as_array()
        .expect("Invalid ABI format")
        .iter()
        .filter(|entry| entry["type"] == "event")
        .collect();
    
    for log in receipt_logs {
        println!("\nProcessing log with topic: {:?}", log.topics[0]);
        
        for event in &events {
            let event_name = event["name"].as_str().unwrap();
            let inputs = event["inputs"].as_array().unwrap();
            
            // Build the event signature string
            let params: Vec<String> = inputs.iter()
                .map(|input| format!("{}", input["type"].as_str().unwrap()))
                .collect();
            let event_sig = format!("{}({})", event_name, params.join(","));
            
            println!("Checking event: {}", event_sig);
            let event_hash = ethers::utils::keccak256(event_sig.as_bytes());
            
            if H256::from(event_hash) == log.topics[0] {
                println!("\nFound matching event: {}", event_name);
                println!("Log Address: {}", log.address);
                
                // Create ParamType vector for decoding
                let param_types: Vec<ParamType> = inputs.iter()
                    .map(|input| {
                        let param_type = input["type"].as_str().unwrap();
                        match param_type {
                            "uint32" => ParamType::Uint(32),
                            "uint256" => ParamType::Uint(256),
                            "string" => ParamType::String,
                            "address" => ParamType::Address,
                            "uint8[]" => ParamType::Array(Box::new(ParamType::Uint(8))),
                            "uint256[]" => ParamType::Array(Box::new(ParamType::Uint(256))),
                            _ => panic!("Unsupported parameter type: {}", param_type),
                        }
                    })
                    .collect();

                // Get parameter names for output
                let param_names: Vec<&str> = inputs.iter()
                    .map(|input| input["name"].as_str().unwrap())
                    .collect();

                // Remove 0x prefix and decode hex data
                let data = hex::decode(&log.data[2..]).unwrap();
                
                match event_name {
                    "DestinationChainUpdated" => {
                        // Existing decoding logic for DestinationChainUpdated
                        println!("\nDecoded Parameters:");
                        let chain_id = U256::from_big_endian(&log.topics[1].as_bytes()).as_u32();
                        println!("chainId (indexed): {}", chain_id);

                        // Get offsets from first 4 32-byte chunks
                        let rpc_url_offset = U256::from_big_endian(&data[0..32]).as_usize();
                        let contract_addr = format!("0x{}", hex::encode(&data[32..64]));
                        let types_offset = U256::from_big_endian(&data[64..96]).as_usize();
                        let fees_offset = U256::from_big_endian(&data[96..128]).as_usize();

                        // Decode RPC URL (string)
                        let str_len = U256::from_big_endian(&data[rpc_url_offset..rpc_url_offset + 32]).as_usize();
                        let rpc_url = String::from_utf8_lossy(&data[rpc_url_offset + 32..rpc_url_offset + 32 + str_len]);
                        println!("rpcUrl: {} (length: {})", rpc_url, str_len);
                        println!("contractAddress: {}", contract_addr);

                        // Decode types array
                        let types_len = U256::from_big_endian(&data[types_offset..types_offset + 32]).as_usize();
                        println!("Transaction Types: [");
                        for i in 0..types_len {
                            let val = U256::from_big_endian(&data[types_offset + 32 + i*32..types_offset + 64 + i*32]);
                            println!("    {}: {}", i, val);
                        }
                        println!("]");

                        // Decode fees array
                        let fees_len = U256::from_big_endian(&data[fees_offset..fees_offset + 32]).as_usize();
                        println!("Transaction Fees: [");
                        for i in 0..fees_len {
                            let val = U256::from_big_endian(&data[fees_offset + 32 + i*32..fees_offset + 64 + i*32]);
                            println!("    Type {}: {} wei", i, val);
                        }
                        println!("]");

                        // Print raw data chunks for debugging
                        println!("\nRaw data chunks:");
                        for (i, chunk) in data.chunks(32).enumerate() {
                            println!("Chunk {}: 0x{}", i, hex::encode(chunk));
                        }
                    }
                    "ETHSentToDestinationChain" => {
                        println!("\nDecoded Parameters:");
                        
                        // First parameter (source chain ID) is indexed
                        let source_chain_id = U256::from_big_endian(&log.topics[1].as_bytes()).as_u32();
                        println!("Source Chain ID (indexed): {}", source_chain_id);

                        // Print raw data first for debugging
                        println!("\nRaw data chunks:");
                        for (i, chunk) in data.chunks(32).enumerate() {
                            println!("Chunk {}: 0x{}", i, hex::encode(chunk));
                        }

                        // Make sure we have enough data
                        if data.len() >= 64 {  // At least 2 chunks of 32 bytes
                            // Decode non-indexed parameters from data
                            let source_addr = format!("0x{}", hex::encode(&data[12..32]));  // address is 20 bytes
                            let dest_addr = format!("0x{}", hex::encode(&data[44..64]));    // address is 20 bytes
                            
                            if data.len() >= 96 {  // At least 3 chunks
                                let dest_chain_id = U256::from_big_endian(&data[64..96]).as_u32();
                                println!("Destination Chain ID: {}", dest_chain_id);
                            }
                            
                            if data.len() >= 128 {  // At least 4 chunks
                                let amount = U256::from_big_endian(&data[96..128]);
                                println!("Amount: {} wei", amount);
                            }

                            println!("Source Address: {}", source_addr);
                            println!("Destination Address: {}", dest_addr);
                        } else {
                            println!("Warning: Not enough data to decode all parameters");
                            println!("Data length: {} bytes", data.len());
                        }
                    }
                    _ => {
                        println!("Decoding not implemented for event: {}", event_name);
                    }
                }
                break;
            }
        }
    }
}
