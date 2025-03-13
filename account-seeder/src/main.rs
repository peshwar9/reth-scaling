// src/main.rs for account-seeder

use clap::Parser;
use ethers::{
    core::types::{TransactionRequest, U256, H160},
    providers::{Http as EthersHttp, Middleware, Provider as EthersProvider},
    signers::{LocalWallet, Signer},
    utils::format_ether,
};
use futures::future::join_all;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::time;

// CLI argument parsing
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// RPC endpoint URL
    #[clap(short = 'u', long, default_value = "http://57.128.116.154:8545")]
    rpc_url: String,

    /// Number of accounts to generate and seed
    #[clap(short = 'n', long, default_value_t = 3000)]
    account_count: usize,

    /// Amount of ETH to send to each account
    #[clap(short = 'a', long, default_value_t = 1.0)]
    amount_eth: f64,

    /// Funder private key (must have sufficient ETH)
    #[clap(short = 'f', long, default_value = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")]
    funder_key: String,

    /// Maximum concurrent transactions
    #[clap(short = 'c', long, default_value_t = 50)]
    concurrency: usize,

    /// Batch size for transaction submission
    #[clap(short = 'b', long, default_value_t = 50)]
    batch_size: usize,

    /// Output file for accounts
    #[clap(short = 'o', long, default_value = "accounts.json")]
    output_file: String,
}

// Account structure
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Account {
    address: String,
    private_key: String,
}

// Error handling
#[derive(Debug, Error)]
enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Provider error: {0}")]
    Provider(String),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    
    #[error("Address parse error: {0}")]
    AddressParse(String),
    
    #[error("Semaphore error: {0}")]
    Semaphore(#[from] tokio::sync::AcquireError),
    
    #[error("Wallet error: {0}")]
    Wallet(#[from] ethers::signers::WalletError),
    
    #[error("Conversion error: {0}")]
    Conversion(#[from] ethers::utils::ConversionError),
    
    #[error("{0}")]
    Other(String),
}

// We don't need a generic From implementation since we already have specific ones
// Just add implementations for ethers errors we'll handle
impl From<ethers::providers::ProviderError> for AppError {
    fn from(err: ethers::providers::ProviderError) -> Self {
        AppError::Provider(err.to_string())
    }
}

impl From<ethers::contract::ContractError<ethers::providers::Provider<EthersHttp>>> for AppError {
    fn from(err: ethers::contract::ContractError<ethers::providers::Provider<EthersHttp>>) -> Self {
        AppError::Provider(err.to_string())
    }
}

type Result<T> = std::result::Result<T, AppError>;

// Helper function to parse an address string without ENS resolution
fn parse_address(address: &str) -> Result<H160> {
    let addr = address.trim_start_matches("0x");
    match H160::from_str(addr) {
        Ok(addr) => Ok(addr),
        Err(e) => Err(AppError::AddressParse(format!("Failed to parse address: {}", e))),
    }
}

// Generate random accounts
fn generate_accounts(count: usize) -> Vec<Account> {
    let mut rng = StdRng::from_entropy();
    
    println!("Generating {} accounts...", count);
    let mut accounts = Vec::with_capacity(count);
    for i in 0..count {
        let wallet = LocalWallet::new(&mut rng);
        accounts.push(Account {
            address: format!("{:x}", wallet.address()),
            private_key: hex::encode(wallet.signer().to_bytes()),
        });
        
        if (i + 1) % 500 == 0 {
            println!("Generated {} accounts", i + 1);
        }
    }
    
    accounts
}

// Save accounts to file
fn save_accounts(senders: &[Account], receivers: &[Account], filename: &str) -> Result<()> {
    let combined = serde_json::json!({
        "senders": senders,
        "receivers": receivers,
    });
    
    let file = File::create(filename)?;
    serde_json::to_writer_pretty(file, &combined)?;
    
    println!("Accounts saved to {}", filename);
    Ok(())
}

// Fund accounts from a funded source account
async fn fund_accounts(
    provider: Arc<EthersProvider<EthersHttp>>,
    funder_wallet: LocalWallet,
    accounts: &[Account],
    amount_wei: U256,
    batch_size: usize,
    concurrency: usize,
) -> Result<()> {
    // Check funder balance
    let funder_address = funder_wallet.address();
    let funder_balance = provider.get_balance(funder_address, None).await?;
    
    // Calculate total needed (using U256 multiplication)
    let total_needed = amount_wei * U256::from(accounts.len());
    
    println!("Funder address: {:?}", funder_address);
    println!("Funder balance: {} ETH", format_ether(funder_balance));
    println!("Total needed: {} ETH", format_ether(total_needed));
    
    if funder_balance < total_needed {
        return Err(AppError::Other(format!(
            "Insufficient funds. Have {} ETH, need {} ETH", 
            format_ether(funder_balance), 
            format_ether(total_needed)
        )));
    }
    
    // Get current gas price
    let base_gas_price = provider.get_gas_price().await?;
    println!("Base gas price: {} gwei", base_gas_price / U256::exp10(9));

    // Get current nonce
    let mut nonce = provider.get_transaction_count(funder_address, None).await?;
    println!("Starting with nonce: {}", nonce);
    
    // Fund accounts in batches
    let batch_count = (accounts.len() + batch_size - 1) / batch_size;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    
    let funder_wallet = funder_wallet.with_chain_id(provider.get_chainid().await?.as_u64());
    let client = ethers::middleware::SignerMiddleware::new(provider.clone(), funder_wallet);
    
    println!("Funding accounts in {} batches...", batch_count);
    
    for batch_idx in 0..batch_count {
        let start_idx = batch_idx * batch_size;
        let end_idx = std::cmp::min(start_idx + batch_size, accounts.len());
        let actual_batch_size = end_idx - start_idx;
        
        println!("Processing batch {}/{} ({} accounts)", batch_idx + 1, batch_count, actual_batch_size);
        
        let mut futures = Vec::with_capacity(actual_batch_size);
        
        for i in 0..actual_batch_size {
            let permit = semaphore.clone().acquire_owned().await?;
            let account_idx = start_idx + i;
            let account = &accounts[account_idx];
            let client = client.clone();
            let current_nonce = nonce;
            
            // Calculate gas price with increasing multiplier based on position in batch
            let gas_price = base_gas_price + (base_gas_price * U256::from(i as u64) / U256::from(10));
            
            // Increment nonce for next transaction
            nonce = nonce + 1;
            
            let future = async move {
                let addr = account.address.trim_start_matches("0x");
                let to_address = match H160::from_str(addr) {
                    Ok(address) => address,
                    Err(e) => {
                        return Err(AppError::AddressParse(format!(
                            "Failed to parse address {}: {}", account.address, e
                        )));
                    }
                };
                
                // Create transaction with calculated gas price
                let tx = TransactionRequest::new()
                    .to(to_address)
                    .value(amount_wei)
                    .gas(21_000)
                    .gas_price(gas_price)
                    .nonce(current_nonce);
                
                let start = Instant::now();
                
                // Send transaction
                match client.send_transaction(tx, None).await {
                    Ok(pending_tx) => {
                        match pending_tx.await {
                            Ok(Some(receipt)) => {
                                let elapsed = start.elapsed();
                                println!("Funded account {}/{} in {:?} (tx: {:?})", 
                                         account_idx + 1, accounts.len(), elapsed, receipt.transaction_hash);
                                Ok(account_idx)
                            },
                            Ok(None) => {
                                println!("Funding transaction for account {} was submitted but not mined yet", account_idx + 1);
                                Ok(account_idx)
                            },
                            Err(e) => {
                                println!("Failed to confirm funding of account {}: {}", account_idx + 1, e);
                                Err(AppError::Provider(format!("Transaction confirmation error: {}", e)))
                            }
                        }
                    },
                    Err(e) => {
                        println!("Failed to fund account {}: {}", account_idx + 1, e);
                        Err(AppError::Provider(format!("Transaction submission error: {}", e)))
                    }
                }?;
                
                drop(permit);
                Ok(())
            };
            
            futures.push(future);
        }
        
        // Wait for all transactions in this batch
        let results: Vec<Result<()>> = join_all(futures).await;
        
        // Check for errors
        for result in results {
            if let Err(e) = result {
                eprintln!("Error in batch: {}", e);
            }
        }
        
        // Increase delay between batches to allow more time for transactions to be mined
        if batch_idx < batch_count - 1 {
            time::sleep(Duration::from_secs(2)).await;
        }
    }
    
    println!("Funded {} accounts with {} ETH each", accounts.len(), format_ether(amount_wei));
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Create provider
    let provider = EthersProvider::try_from(args.rpc_url.clone())?;
    let provider = Arc::new(provider);
    
    // Get chain ID
    let chain_id = provider.get_chainid().await?.as_u64();
    println!("Connected to chain ID: {}", chain_id);
    
    // Generate accounts
    let sender_accounts = generate_accounts(args.account_count);
    let receiver_accounts = generate_accounts(args.account_count);
    
    // Parse funder private key
    let funder_key = if args.funder_key.starts_with("0x") {
        args.funder_key.clone()
    } else {
        format!("0x{}", args.funder_key)
    };
    
    let funder_wallet = funder_key.parse::<LocalWallet>()?;
    println!("Funder address: {:?}", funder_wallet.address());
    
    // Calculate amount in wei
    let amount_wei = ethers::utils::parse_ether(args.amount_eth)?;
    
    // Fund sender accounts
    println!("Funding {} sender accounts with {} ETH each...", 
             sender_accounts.len(), args.amount_eth);
    fund_accounts(
        provider.clone(),
        funder_wallet,
        &sender_accounts,
        amount_wei,
        args.batch_size,
        args.concurrency
    ).await?;
    
    // Save accounts to file
    save_accounts(&sender_accounts, &receiver_accounts, &args.output_file)?;
    
    println!("Account seeding completed successfully!");
    println!("You can now run the transaction generator with:");
    println!("cargo run --bin tx-generator -- --tx-count {} --batch-size 50 --concurrency 50 --target-tps 3000 --use-batching --accounts-file {}", 
             args.account_count, args.output_file);
    
    Ok(())
}