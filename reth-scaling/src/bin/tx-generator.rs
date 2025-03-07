// src/main.rs for tx-generator

use clap::Parser;
use ethers::{
    core::types::{Address as EthersAddress, TransactionRequest, U256},
    providers::{Http as EthersHttp, Middleware, Provider as EthersProvider},
    signers::{LocalWallet, Signer},
};
use futures::future::join_all;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{sync::Semaphore, time};

// CLI argument parsing
#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// RPC endpoint URL
    #[clap(short = 'u', long, default_value = "http://localhost:8545")]
    rpc_url: String,

    /// Number of sender accounts to generate
    #[clap(short = 's', long, default_value_t = 3000)]
    sender_count: usize,

    /// Number of receiver accounts to generate
    #[clap(short = 'r', long, default_value_t = 3000)]
    receiver_count: usize,

    /// Generate accounts only (no transactions)
    #[clap(long)]
    gen_accounts: bool,

    /// Total number of transactions to send
    #[clap(short = 't', long, default_value_t = 10000)]
    tx_count: usize,

    /// Maximum concurrent transactions
    #[clap(short = 'c', long, default_value_t = 100)]
    concurrency: usize,

    /// Batch size for transaction submission
    #[clap(short = 'b', long, default_value_t = 100)]
    batch_size: usize,

    /// Use JSON-RPC batching
    #[clap(long)]
    use_batching: bool,

    /// Target TPS (0 = max speed)
    #[clap(long, default_value_t = 0)]
    target_tps: usize,

    /// Path to accounts file (if already generated)
    #[clap(long)]
    accounts_file: Option<String>,

    /// Generate genesis file with pre-funded accounts
    #[clap(long)]
    gen_genesis: bool,
}

// Account structure for senders and receivers
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Account {
    address: String,
    private_key: String,
}

// Statistics for transaction processing
#[derive(Debug)]
struct TxStats {
    submitted: usize,
    confirmed: usize,
    failed: usize,
    avg_latency: Duration,
    total_time: Duration,
    tps: f64,
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
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<Box<dyn std::error::Error>> for AppError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        AppError::Other(err.to_string())
    }
}

type Result<T> = std::result::Result<T, AppError>;

// Generate random accounts
async fn generate_accounts(sender_count: usize, receiver_count: usize) -> Result<(Vec<Account>, Vec<Account>)> {
    let mut rng = StdRng::from_entropy();
    
    println!("Generating {} sender accounts...", sender_count);
    let mut senders = Vec::with_capacity(sender_count);
    for _ in 0..sender_count {
        let wallet = LocalWallet::new(&mut rng);
        senders.push(Account {
            address: format!("{:x}", wallet.address()),  // Use lowercase hex without 0x prefix
            private_key: hex::encode(wallet.signer().to_bytes()),  // Clean hex encoding
        });
    }
    
    println!("Generating {} receiver accounts...", receiver_count);
    let mut receivers = Vec::with_capacity(receiver_count);
    for _ in 0..receiver_count {
        let wallet = LocalWallet::new(&mut rng);
        receivers.push(Account {
            address: format!("{:x}", wallet.address()),  // Use lowercase hex without 0x prefix
            private_key: hex::encode(wallet.signer().to_bytes()),  // Clean hex encoding
        });
    }
    
    Ok((senders, receivers))
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

// Load accounts from file
fn load_accounts(filename: &str) -> Result<(Vec<Account>, Vec<Account>)> {
    let file = File::open(filename)?;
    let data: serde_json::Value = serde_json::from_reader(file)?;
    
    let senders: Vec<Account> = serde_json::from_value(data["senders"].clone())?;
    let receivers: Vec<Account> = serde_json::from_value(data["receivers"].clone())?;
    
    println!("Loaded {} senders and {} receivers from {}", senders.len(), receivers.len(), filename);
    Ok((senders, receivers))
}

// Generate genesis file with pre-funded accounts
fn generate_genesis(senders: &[Account], base_genesis: &str, output: &str) -> Result<()> {
    // Read base genesis file
    let file = File::open(base_genesis)?;
    let mut genesis: serde_json::Value = serde_json::from_reader(file)?;
    
    // Add sender accounts with balance
    let alloc = genesis["alloc"].as_object_mut().unwrap();
    for sender in senders {
        // Process address to get the correct format for the genesis file
        let address = if sender.address.starts_with("0x") {
            sender.address[2..].to_string()
        } else {
            sender.address.clone()
        };
        
        // Clean up any extra formatting
        let address = address.trim_start_matches("0x").trim();
        
        let balance = "0x152d02c7e14af6800000"; // 100K ETH in hex
        
        alloc.insert(
            address.to_string(),
            serde_json::json!({
                "balance": balance
            }),
        );
    }
    
    // Write updated genesis file
    let file = File::create(output)?;
    serde_json::to_writer_pretty(file, &genesis)?;
    
    println!("Genesis file with {} funded accounts saved to {}", senders.len(), output);
    Ok(())
}

// Convert a string to an EthersAddress
fn parse_address(address_str: &str) -> Result<EthersAddress> {
    let address_str = address_str.trim();
    let address_str = if address_str.starts_with("0x") {
        address_str.to_string()
    } else {
        // If no 0x prefix, add it
        format!("0x{}", address_str)
    };
    
    EthersAddress::from_str(&address_str)
        .map_err(|e| AppError::Parse(format!("Failed to parse address: {}", e)))
}

// Send a single transaction
async fn send_single_transaction(
    provider: Arc<EthersProvider<EthersHttp>>,
    sender: &Account,
    receiver: &Account,
    nonce: u64,
    chain_id: u64,
    tx_idx: usize,
) -> Result<ethers::types::H256> {
    // Parse receiver address
    let to_address = parse_address(&receiver.address)?;
    
    // Create wallet for signing
    let private_key = if sender.private_key.starts_with("0x") {
        sender.private_key.clone()
    } else {
        format!("0x{}", sender.private_key)
    };
    
    let wallet = private_key.parse::<LocalWallet>()
        .map_err(|e| AppError::Parse(format!("Failed to parse private key: {}", e)))?;
    
    // Set chain ID on the wallet
    let wallet = wallet.with_chain_id(chain_id);
    
    // Create transaction
    let tx = TransactionRequest::new()
        .to(to_address)
        .value(U256::from(1_000_000_000_000_000u64)) // 0.001 ETH
        .gas(21_000)
        .gas_price(U256::from(1_000_000_000u64)) // 1 Gwei
        .nonce(nonce);
    
    // Sign and send transaction
    let client = ethers::middleware::SignerMiddleware::new(provider.clone(), wallet);
    let pending_tx = client.send_transaction(tx, None).await
        .map_err(|e| AppError::Provider(format!("Failed to send transaction: {}", e)))?;
    
    if tx_idx % 1000 == 0 {
        println!("Transaction {} sent: {:?}", tx_idx, pending_tx.tx_hash());
    }
    
    Ok(pending_tx.tx_hash())
}

// Main transaction sending function
async fn send_transactions(
    args: Args,
    senders: Vec<Account>,
    receivers: Vec<Account>,
) -> Result<TxStats> {
    // Create provider
    let provider = EthersProvider::try_from(args.rpc_url.clone())
        .map_err(|e| AppError::Provider(format!("Failed to create provider: {}", e)))?;
    let provider = Arc::new(provider);
    
    // Get chain ID
    let chain_id = provider
        .get_chainid()
        .await
        .map_err(|e| AppError::Provider(format!("Failed to get chain ID: {}", e)))?
        .as_u64();
    
    println!("Connected to chain ID: {}", chain_id);
    
    // Get initial nonces for all senders
    let mut nonces = Vec::with_capacity(senders.len());
    for sender in &senders {
        let address = parse_address(&sender.address)?;
        
        let nonce = provider
            .get_transaction_count(address, None)
            .await
            .map_err(|e| AppError::Provider(format!("Failed to get nonce: {}", e)))?
            .as_u64();
        
        nonces.push(nonce);
    }
    
    let tx_counter = Arc::new(AtomicUsize::new(0));
    let confirmed_counter = Arc::new(AtomicUsize::new(0));
    let failed_counter = Arc::new(AtomicUsize::new(0));
    
    let start_time = Instant::now();
    let semaphore = Arc::new(Semaphore::new(args.concurrency));
    
    let latency_sum = Arc::new(Mutex::new(Duration::from_secs(0)));
    
    println!("Starting transaction generation...");
    println!("Target: {} transactions", args.tx_count);
    
    if args.use_batching {
        // Batch mode
        let batch_count = (args.tx_count + args.batch_size - 1) / args.batch_size;
        let mut handles = Vec::new();
        
        for batch_idx in 0..batch_count {
            let permit = semaphore.clone().acquire_owned().await
                .map_err(|e| AppError::Other(e.to_string()))?;
            
            let provider = provider.clone();
            let tx_counter = tx_counter.clone();
            let confirmed_counter = confirmed_counter.clone();
            let failed_counter = failed_counter.clone();
            let latency_sum = latency_sum.clone();
            let senders = senders.clone();
            let receivers = receivers.clone();
            let mut local_nonces = nonces.clone();
            let batch_size = args.batch_size;
            let tx_count = args.tx_count;
            let target_tps = args.target_tps;
            let chain_id = chain_id;
            
            let batch_start = batch_idx * batch_size;
            let batch_end = std::cmp::min(batch_start + batch_size, tx_count);
            let actual_batch_size = batch_end - batch_start;
            
            let handle = tokio::spawn(async move {
                let start = Instant::now();
                tx_counter.fetch_add(actual_batch_size, Ordering::SeqCst);
                
                if batch_idx % 10 == 0 {
                    println!("Sending batch {}/{} ({} transactions)", 
                             batch_idx + 1, batch_count, actual_batch_size);
                }
                
                let mut futures = Vec::with_capacity(actual_batch_size);
                
                for i in 0..actual_batch_size {
                    let tx_idx = batch_start + i;
                    let sender_idx = tx_idx % senders.len();
                    let receiver_idx = tx_idx % receivers.len();
                    
                    let sender = &senders[sender_idx];
                    let receiver = &receivers[receiver_idx];
                    let nonce = local_nonces[sender_idx];
                    
                    // Create future for sending transaction
                    let future = send_single_transaction(
                        provider.clone(),
                        sender,
                        receiver,
                        nonce,
                        chain_id,
                        tx_idx
                    );
                    
                    // Increment nonce for this sender
                    local_nonces[sender_idx] = nonce + 1;
                    
                    futures.push(future);
                }
                
                // Wait for all transactions in the batch
                let results = join_all(futures).await;
                
                // Process results
                for result in results {
                    match result {
                        Ok(_tx_hash) => {
                            confirmed_counter.fetch_add(1, Ordering::SeqCst);
                        },
                        Err(e) => {
                            eprintln!("Transaction error: {}", e);
                            failed_counter.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
                
                let elapsed = start.elapsed();
                
                // Update latency stats
                {
                    let mut latency = latency_sum.lock().unwrap();
                    *latency += elapsed;
                }
                
                // Rate limiting if target TPS is set
                if target_tps > 0 {
                    let target_batch_time = Duration::from_secs_f64(actual_batch_size as f64 / target_tps as f64);
                    if elapsed < target_batch_time {
                        time::sleep(target_batch_time - elapsed).await;
                    }
                }
                
                drop(permit);
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            });
            
            handles.push(handle);
        }
        
        // Wait for all batches to complete
        for handle in handles {
            handle.await
                .map_err(|e| AppError::Other(e.to_string()))?
                .map_err(|e| AppError::Other(e.to_string()))?;
        }
    } else {
        // Individual transaction mode
        let mut handles = Vec::new();
        let nonces = Arc::new(Mutex::new(nonces));
        
        for tx_idx in 0..args.tx_count {
            let permit = semaphore.clone().acquire_owned().await
                .map_err(|e| AppError::Other(e.to_string()))?;
            
            let provider = provider.clone();
            let tx_counter = tx_counter.clone();
            let confirmed_counter = confirmed_counter.clone();
            let failed_counter = failed_counter.clone();
            let latency_sum = latency_sum.clone();
            let target_tps = args.target_tps;
            let nonces = nonces.clone();
            
            let sender_idx = tx_idx % senders.len();
            let receiver_idx = tx_idx % receivers.len();
            
            let sender = senders[sender_idx].clone();
            let receiver = receivers[receiver_idx].clone();
            let chain_id = chain_id;
            
            let handle = tokio::spawn(async move {
                let start = Instant::now();
                tx_counter.fetch_add(1, Ordering::SeqCst);
                
                // Get and update nonce
                let nonce = {
                    let mut nonces_guard = nonces.lock().unwrap();
                    let nonce = nonces_guard[sender_idx];
                    nonces_guard[sender_idx] = nonce + 1;
                    nonce
                };
                
                // Send transaction
                match send_single_transaction(
                    provider.clone(),
                    &sender,
                    &receiver,
                    nonce,
                    chain_id,
                    tx_idx
                ).await {
                    Ok(_tx_hash) => {
                        confirmed_counter.fetch_add(1, Ordering::SeqCst);
                    },
                    Err(e) => {
                        eprintln!("Transaction error: {}", e);
                        failed_counter.fetch_add(1, Ordering::SeqCst);
                    }
                }
                
                let elapsed = start.elapsed();
                
                // Update latency stats
                {
                    let mut latency = latency_sum.lock().unwrap();
                    *latency += elapsed;
                }
                
                // Rate limiting if target TPS is set
                if target_tps > 0 {
                    let target_tx_time = Duration::from_secs_f64(1.0 / target_tps as f64);
                    if elapsed < target_tx_time {
                        time::sleep(target_tx_time - elapsed).await;
                    }
                }
                
                drop(permit);
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            });
            
            handles.push(handle);
        }
        
        // Wait for all transactions to complete
        for handle in handles {
            handle.await
                .map_err(|e| AppError::Other(e.to_string()))?
                .map_err(|e| AppError::Other(e.to_string()))?;
        }
    }
    
    let total_time = start_time.elapsed();
    let tps = args.tx_count as f64 / total_time.as_secs_f64();
    
    let submitted = tx_counter.load(Ordering::SeqCst);
    let confirmed = confirmed_counter.load(Ordering::SeqCst);
    let failed = failed_counter.load(Ordering::SeqCst);
    
    let latency = *latency_sum.lock().unwrap();
    let avg_latency = if submitted > 0 {
        latency / submitted as u32
    } else {
        Duration::from_secs(0)
    };
    
    Ok(TxStats {
        submitted,
        confirmed,
        failed,
        avg_latency,
        total_time,
        tps,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Generate or load accounts
    let (senders, receivers) = if let Some(accounts_file) = &args.accounts_file {
        load_accounts(accounts_file)?
    } else {
        let (s, r) = generate_accounts(args.sender_count, args.receiver_count).await?;
        save_accounts(&s, &r, "accounts.json")?;
        (s, r)
    };
    
    // Generate genesis file if requested
    if args.gen_genesis {
        generate_genesis(&senders, "dev-genesis.json", "funded-genesis.json")?;
        println!("To use this genesis, start RETH with: --genesis=funded-genesis.json");
    }
    
    // Exit if we only needed to generate accounts
    if args.gen_accounts {
        println!("Account generation completed.");
        return Ok(());
    }
    
    // Send transactions and measure performance
    let stats = send_transactions(args.clone(), senders, receivers).await?;
    
    // Print results
    println!("\n=== Transaction Test Results ===");
    println!("Total transactions submitted: {}", stats.submitted);
    println!("Transactions confirmed: {}", stats.confirmed);
    println!("Transactions failed: {}", stats.failed);
    println!("Total time: {:.2?}", stats.total_time);
    println!("Average transaction latency: {:.2?}", stats.avg_latency);
    println!("Throughput: {:.2} TPS", stats.tps);
    
    // Save statistics to file
    let stats_json = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "submitted": stats.submitted,
        "confirmed": stats.confirmed,
        "failed": stats.failed,
        "total_time_ms": stats.total_time.as_millis(),
        "avg_latency_ms": stats.avg_latency.as_millis(),
        "tps": stats.tps,
        "config": {
            "target_tps": args.target_tps,
            "concurrency": args.concurrency,
            "batch_size": args.batch_size,
            "use_batching": args.use_batching,
        }
    });
    
    let stats_file = File::create("tx_stats.json")?;
    serde_json::to_writer_pretty(stats_file, &stats_json)?;
    println!("Statistics saved to tx_stats.json");
    
    Ok(())
}