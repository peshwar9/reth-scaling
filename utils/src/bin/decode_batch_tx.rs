use ethers::{
    prelude::*,
    providers::{Provider, Http, Middleware},
    types::{Transaction, H256, U256, Block},
    abi::{Function, Token},
};
use eyre::Result;
use std::{env, time::Duration, sync::Arc};
use serde_json::Value;
use tokio::time::sleep;

#[derive(Debug)]
struct BatchTransferInfo {
    tx_hash: H256,
    num_transfers: usize,
    gas_used: U256,
    gas_price: U256,
    total_eth_transferred: U256,
    source_chain_id: u32,
    block_number: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Get RPC URL and contract address from env
    let rpc_url = env::var("NODE4_RPC")?;
    let contract_addr = env::var("NODE4_CONTRACT")?
        .parse::<Address>()?;

    println!("Connecting to RPC endpoint: {}", rpc_url);
    println!("Monitoring contract: {:#x}", contract_addr);

    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);

    // Load contract ABI
    let abi_content = include_str!("abi.json");
    let abi_json: Value = serde_json::from_str(abi_content)?;
    let abi: ethers::abi::Abi = serde_json::from_value(abi_json["abi"].clone())?;

    let func = abi.function("receiveETHfromSourceChainInBatch")
        .expect("Function not found in ABI");
    let func_sig = func.short_signature();

    println!("\nStarting block monitoring...");
    println!("Looking for batch transfers (function signature: 0x{})...", hex::encode(func_sig));

    let mut block_number = client.get_block_number().await?;
    println!("Starting from block: {}", block_number);
    
    loop {
        let latest_block = client.get_block_number().await?;
        
        while block_number <= latest_block {
            print!("\rChecking block {} for batch transfers...", block_number);

            if let Some(block) = client.get_block_with_txs(block_number).await? {
                if !block.transactions.is_empty() {
                    println!("\nBlock {} has {} transactions", block_number, block.transactions.len());
                    process_block(&client, &block, contract_addr, func_sig, func).await?;
                }
            }
            block_number += 1.into();
        }

        // Clear line before sleeping
        print!("\rWaiting for new blocks...");
        sleep(Duration::from_millis(1000)).await;
    }
}

async fn process_block(
    client: &Provider<Http>,
    block: &Block<Transaction>,
    contract_addr: Address,
    func_sig: [u8; 4],
    func: &Function,
) -> Result<()> {
    let block_number = block.number.unwrap_or_default();

    for tx in &block.transactions {
        if tx.to == Some(contract_addr) {
            if tx.input.0.len() >= 4 && tx.input.0[0..4] == func_sig {
                // Found a matching transaction, decode it
                if let Ok(decoded) = func.decode_input(&tx.input.0[4..]) {
                    // Get recipients array length (3rd parameter)
                    if let Some(Token::Array(recipients)) = decoded.get(2) {
                        // Get gas usage from receipt
                        if let Some(receipt) = client.get_transaction_receipt(tx.hash).await? {
                            println!("\nBlock {} - Found batch transfer:", block_number);
                            println!("  Transaction: {:#x}", tx.hash);
                            println!("  Number of transfers in batch: {}", recipients.len());
                            println!("  Gas used: {}", receipt.gas_used.unwrap_or_default());
                        }
                    }
                }
            }
        }
    }

    Ok(())
} 