use clap::{Parser, Subcommand};
use ethers::{
    prelude::*,
    types::{Address, TransactionRequest, transaction::eip2718::TypedTransaction, U256},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, sync::Arc};
use tokio::time::Instant;
use dotenv::dotenv;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Prepare {
        #[arg(long)]
        num_accounts: usize,
        #[arg(long)]
        num_nodes: usize,
    },
    /// Defund all accounts in a specific node back to master wallet
    DefundNode {
        #[arg(long)]
        node: usize,
    },
    /// Send ETH cross-chain one-way between two nodes
    #[command(name = "send-eth-1way")]
    SendEth1way {
        #[arg(long)]
        from_node: usize,  // Source node ID
        #[arg(long)]
        to_node: usize,    // Destination node ID
        #[arg(long)]
        num_accounts: usize,
        #[arg(long)]
        amount_wei: U256,
        #[arg(long)]
        rounds: usize,
    },
    /// Fund sender accounts of a specific node
    FundNode {
        #[arg(long)]
        node: usize,
        #[arg(long)]
        amount_eth: f64,
    },
    /// Get balances of all sender accounts for a specific node
    NodeBalances {
        #[arg(long)]
        node: usize,
    },
    /// Send ETH cross-chain N-way between all nodes
    #[command(name = "send-eth-nway")]
    SendEthNway {
        #[arg(long)]
        num_nodes: usize,
        #[arg(long)]
        num_accounts: usize,
        #[arg(long)]
        amount_wei: U256,
        #[arg(long)]
        rounds: String,  // String to handle both numbers and '#'
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Account {
    private_key: String,
    address: String,
}

fn main() {
    dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Prepare { num_accounts, num_nodes } => {
            prepare_node_accounts(num_accounts, num_nodes);
        }
        Commands::DefundNode { node } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(defund_node(node)) {
                eprintln!("Error defunding node {}: {}", node, err);
            }
        }
        Commands::SendEth1way { from_node, to_node, num_accounts, amount_wei, rounds } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(send_eth_crosschain(
                from_node, to_node, num_accounts, amount_wei, rounds
            )) {
                eprintln!("Error sending cross-chain ETH: {}", err);
            }
        }
        Commands::FundNode { node, amount_eth } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(fund_node(node, amount_eth)) {
                eprintln!("Error funding node {}: {}", node, err);
            }
        }
        Commands::NodeBalances { node } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(check_node_balances(node)) {
                eprintln!("Error checking balances for node {}: {}", node, err);
            }
        }
        Commands::SendEthNway { num_nodes, num_accounts, amount_wei, rounds } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(send_eth_crosschain_loop(num_nodes, num_accounts, amount_wei, &rounds)) {
                eprintln!("Error in N-way ETH transfer: {}", err);
            }
        }
    }
}

// Helper function to format Wei to ETH
fn format_eth(wei: U256) -> String {
    let eth = wei.as_u128() as f64 / 1e18;
    format!("{:.6}", eth)
}

fn prepare_node_accounts(accounts_per_node: usize, num_nodes: usize) {
    // Read the accounts.json file
    let accounts_file = fs::read_to_string("../accounts.json")
        .expect("Failed to read accounts.json");
    let accounts: Value = serde_json::from_str(&accounts_file)
        .expect("Failed to parse accounts.json");

    let accounts_array = accounts.as_array()
        .expect("Accounts should be an array");

    // Calculate total accounts needed
    let total_accounts_needed = accounts_per_node * 2 * num_nodes;
    if accounts_array.len() < total_accounts_needed {
        panic!("Not enough accounts in accounts.json. Need {} accounts but found {}", 
            total_accounts_needed, accounts_array.len());
    }

    // Create node files
    for node_idx in 0..num_nodes {
        let start_idx = node_idx * accounts_per_node * 2;
        
        // Get sender accounts
        let senders: Vec<Value> = accounts_array[start_idx..start_idx + accounts_per_node]
            .to_vec();
        
        // Get receiver accounts
        let receivers: Vec<Value> = accounts_array[start_idx + accounts_per_node..start_idx + accounts_per_node * 2]
            .to_vec();

        // Create node configuration
        let node_config = json!({
            "senders": senders,
            "receivers": receivers
        });

        // Write to file
        let filename = format!("node-{}.json", node_idx + 1);
        fs::write(
            &filename,
            serde_json::to_string_pretty(&node_config).unwrap()
        ).expect(&format!("Failed to write {}", filename));

        println!("Created {}", filename);
    }
}

async fn defund_node(node: usize) -> eyre::Result<()> {
    // Get master wallet address from .env
    let master_address = env::var("MASTER_WALLET_ADDRESS")
        .expect("MASTER_WALLET_ADDRESS must be set in .env file");
    let master_address: Address = master_address.parse()?;

    // Get node-specific RPC URL
    let rpc_url = env::var(format!("NODE{}_RPC", node))
        .map_err(|_| eyre::eyre!("NODE{}_RPC not set in .env", node))?;
    
    // Connect to network
    let provider = Provider::<Http>::try_from(rpc_url.clone())?;
    let client = Arc::new(provider);

    // Read node file
    let filename = format!("node-{}.json", node);
    let file_content = fs::read_to_string(&filename)?;
    let node_data: Value = serde_json::from_str(&file_content)?;
    
    println!("Starting to defund Node {} accounts...", node);
    println!("Using RPC URL: {}", rpc_url);
    println!("Master wallet address: {}", master_address);
    
    let start_time = Instant::now();
    let mut total_defunded = 0;
    let mut total_failed = 0;

    // Process both senders and receivers
    for account_type in ["senders", "receivers"] {
        let accounts = node_data[account_type].as_array()
            .ok_or_else(|| eyre::eyre!("{} not found in {}", account_type, filename))?;

        println!("\nProcessing {} accounts...", account_type);
        
        for (idx, account) in accounts.iter().enumerate() {
            let private_key = account["private_key"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid private key format"))?;
            let wallet = private_key.parse::<LocalWallet>()?;
            let wallet = wallet.with_chain_id(client.get_chainid().await?.as_u64());
            
            let address = wallet.address();
            let balance = client.get_balance(address, None).await?;
            
            if balance > U256::zero() {
                println!("\nDefunding {} account {} ({})...", account_type, idx + 1, address);
                println!("  Current balance: {} wei ({} ETH)", balance, format_eth(balance));

                // Calculate gas cost for transfer
                let gas_price = U256::zero();  // Using zero gas price
                let gas_limit = U256::from(21_000);
                let gas_cost = gas_price * gas_limit;
                
                // Send entire balance minus gas cost
                let transfer_amount = balance - gas_cost;
                
                if transfer_amount > U256::zero() {
                    // Get the current nonce for this account
                    let nonce = client.get_transaction_count(address, None).await?;
                    
                    let tx = TransactionRequest::new()
                        .to(master_address)
                        .value(transfer_amount)
                        .from(address)
                        .gas(gas_limit)
                        .gas_price(gas_price)
                        .nonce(nonce);  // Add the current nonce

                    let typed_tx = TypedTransaction::Legacy(tx);
                    match wallet.sign_transaction(&typed_tx).await {
                        Ok(signature) => {
                            let signed_tx = typed_tx.rlp_signed(&signature);
                            match client.send_raw_transaction(signed_tx).await {
                                Ok(tx_hash) => {
                                    println!("✓ Transaction successful!");
                                    println!("  Transaction hash: 0x{:x}", tx_hash.tx_hash());
                                    println!("  Amount transferred: {} wei ({} ETH)", 
                                        transfer_amount, format_eth(transfer_amount));
                                    total_defunded += 1;
                                }
                                Err(e) => {
                                    println!("✗ Transaction failed!");
                                    println!("  Error: {}", e);
                                    total_failed += 1;
                                }
                            }
                        }
                        Err(e) => {
                            println!("✗ Failed to sign transaction!");
                            println!("  Error: {}", e);
                            total_failed += 1;
                        }
                    }
                } else {
                    println!("  Skipping: Balance too low to cover gas cost");
                    println!("  Current balance: {} wei", balance);
                    total_failed += 1;
                }
            } else {
                println!("\nSkipping {} account {} ({}): Zero balance", 
                    account_type, idx + 1, address);
            }
        }
    }

    let elapsed = start_time.elapsed();
    println!("\nDefunding Summary for Node {}:", node);
    println!("Total accounts processed: {}", total_defunded + total_failed);
    println!("Successfully defunded: {}", total_defunded);
    println!("Failed/skipped: {}", total_failed);
    println!("Time taken: {:?}", elapsed);

    Ok(())
}

async fn send_eth_crosschain(
    from_node: usize,
    to_node: usize,
    num_accounts: usize,
    amount_wei: U256,
    rounds: usize,
) -> eyre::Result<()> {
    // Get source node's details from .env
    let src_chain_id: u32 = env::var(format!("NODE{}_CHAINID", from_node))?
        .parse()
        .map_err(|_| eyre::eyre!("Invalid chain ID format for NODE{}_CHAINID", from_node))?;
    
    let contract_addr = env::var(format!("NODE{}_CONTRACT", from_node))?
        .parse::<Address>()
        .map_err(|_| eyre::eyre!("Invalid contract address for NODE{}_CONTRACT", from_node))?;
    
    let rpc_url = env::var(format!("NODE{}_RPC", from_node))?;

    // Get destination chain ID
    let dst_chain_id: u32 = env::var(format!("NODE{}_CHAINID", to_node))?
        .parse()
        .map_err(|_| eyre::eyre!("Invalid chain ID format for NODE{}_CHAINID", to_node))?;

    // Store transaction info for later verification
    #[derive(Debug)]
    struct TxInfo {
        round: usize,
        hash: H256,
        from_chain: u32,
        to_chain: u32,
        from_addr: Address,
        to_addr: Address,
        amount: U256,
    }
    let mut transactions = Vec::new();

    println!("Starting cross-chain ETH transfers...");
    let start_time = Instant::now();
    let mut total_sent = 0;
    let expected_total = rounds * num_accounts; // Add this to track expected total

    // Read source and destination node files
    let src_filename = format!("node-{}.json", from_node);
    let src_content = fs::read_to_string(&src_filename)?;
    let src_data: Value = serde_json::from_str(&src_content)?;

    let dst_filename = format!("node-{}.json", to_node);
    let dst_content = fs::read_to_string(&dst_filename)?;
    let dst_data: Value = serde_json::from_str(&dst_content)?;

    // Connect to source node's network
    let provider = Provider::<Http>::try_from(rpc_url.clone())?;
    let client = Arc::new(provider);
    
    // Get chain ID early
    let chain_id = client.get_chainid().await?;
    println!("Connected to network. Chain ID: {}", chain_id);

    // Create contract instance
    let contract_json: Value = serde_json::from_slice(
        include_bytes!("../../../reth-contract/out/MonetSmartContract.sol/MonetSmartContract.json")
    )?;
    let abi: ethers::abi::Abi = serde_json::from_value(contract_json["abi"].clone())?;

    // Track nonces for each sender
    let mut sender_nonces: HashMap<Address, U256> = HashMap::new();

    // Process each round
    for round in 1..=rounds {
        println!("\nStarting round {}/{}", round, rounds);

        // Process each account
        for acc_idx in 0..num_accounts {
            let sender = &src_data["senders"][acc_idx];
            let sender_key = sender["private_key"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid private key format"))?;
            
            // Set chain ID when creating wallet
            let sender_wallet = sender_key.parse::<LocalWallet>()?
                .with_chain_id(chain_id.as_u64());
            
            let receiver = &dst_data["receivers"][acc_idx];
            let receiver_addr = receiver["address"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid receiver address"))?
                .parse::<Address>()?;

            println!("\nTransaction Details:");
            println!("  From Node: {} (Chain ID: {})", from_node, src_chain_id);
            println!("  To Node: {} (Chain ID: {})", to_node, dst_chain_id);
            println!("  Sender Address: {:#x}", sender_wallet.address());
            println!("  Receiver Address: {:#x}", receiver_addr);
            println!("  Amount: {} wei", amount_wei);
            println!("  Contract Address: {:#x}", contract_addr);

            let contract = Contract::new(
                contract_addr,
                abi.clone(),
                Arc::new(SignerMiddleware::new(
                    client.clone(),
                    sender_wallet.clone()
                ))
            );

            // Check balance and send transaction
            let sender_balance = client.get_balance(sender_wallet.address(), None).await?;
            println!("  Sender Balance: {} wei", sender_balance);
            
            let gas_price = U256::zero();
            let gas_limit = U256::from(50_000);
            let total_needed = amount_wei;

            if sender_balance < total_needed {
                println!("✗ Insufficient funds!");
                println!("  Balance: {} wei", sender_balance);
                println!("  Needed: {} wei", total_needed);
                continue;
            }

            println!("Sending transaction...");

            // Get or initialize nonce for this sender
            let nonce = if let Some(n) = sender_nonces.get(&sender_wallet.address()) {
                *n
            } else {
                let n = client.get_transaction_count(sender_wallet.address(), None).await?;
                sender_nonces.insert(sender_wallet.address(), n);
                n
            };

            match contract.method::<_, H256>("sendETHToDestinationChain", (
                dst_chain_id,
                receiver_addr,
            ))?.gas(gas_limit)
              .gas_price(gas_price)
              .value(amount_wei)
              .nonce(nonce)  // Set the nonce explicitly
              .send()
              .await {
                Ok(tx) => {
                    let tx_hash = tx.tx_hash();
                    println!("✓ Transaction sent successfully!");
                    println!("  Transaction hash: {:#x}", tx_hash);
                    
                    // Increment nonce for next use
                    sender_nonces.insert(sender_wallet.address(), nonce + U256::from(1));
                    
                    transactions.push(TxInfo {
                        round,
                        hash: tx_hash,
                        from_chain: src_chain_id,
                        to_chain: dst_chain_id,
                        from_addr: sender_wallet.address(),
                        to_addr: receiver_addr,
                        amount: amount_wei,
                    });
                    total_sent += 1;
                }
                Err(e) => {
                    println!("✗ Transaction failed to send!");
                    println!("  Error: {}", e);
                    println!("  Sender: {:#x}", sender_wallet.address());
                    println!("  Chain ID used: {}", chain_id);
                }
            }
        }
    }

    println!("\nAll transactions sent. Waiting for receipts...");

    // Create log file
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("eth-transfer-1way.log")?;
    let mut log = BufWriter::new(log_file);

    // Wait for all transaction receipts with timeout
    let max_wait = Duration::from_secs(60); // Maximum wait time of 60 seconds
    let start_wait = Instant::now();

    while !transactions.is_empty() && start_wait.elapsed() < max_wait {
        let mut completed = Vec::new();
        
        for (idx, tx_info) in transactions.iter().enumerate() {
            match client.get_transaction_receipt(tx_info.hash).await? {
                Some(receipt) => {
                    let status = if receipt.status.unwrap().as_u64() == 1 {
                        "success"
                    } else {
                        "failed"
                    };

                    writeln!(log, "{},{},{:#x},{},{},{:#x},{:#x},{}",
                        status,
                        tx_info.round,
                        tx_info.hash,
                        tx_info.from_chain,
                        tx_info.to_chain,
                        tx_info.from_addr,
                        tx_info.to_addr,
                        tx_info.amount
                    )?;
                    completed.push(idx);
                }
                None => {
                    // Transaction still pending
                    continue;
                }
            }
        }

        // Remove completed transactions from back to front
        for idx in completed.iter().rev() {
            transactions.remove(*idx);
        }

        if !transactions.is_empty() {
            sleep(Duration::from_secs(1)).await;
        }
    }

    // Log any remaining transactions as pending
    for tx_info in transactions {
        writeln!(log, "pending,{},{:#x},{},{},{:#x},{:#x},{}",
            tx_info.round,
            tx_info.hash,
            tx_info.from_chain,
            tx_info.to_chain,
            tx_info.from_addr,
            tx_info.to_addr,
            tx_info.amount
        )?;
    }

    log.flush()?;

    let elapsed = start_time.elapsed();
    println!("\nTransfer Summary:");
    println!("Expected transactions: {}", expected_total);
    println!("Total transactions sent: {}", total_sent);
    println!("Time taken: {:?}", elapsed);

    // Add verification
    if total_sent != expected_total {
        println!("\nWARNING: Not all expected transactions were sent!");
        println!("Expected: {}, Actual: {}", expected_total, total_sent);
    }

    Ok(())
}

async fn fund_node(node: usize, amount_eth: f64) -> eyre::Result<()> {
    // Convert ETH to wei
    let amount_wei = U256::from((amount_eth * 1e18) as u64);
    
    // Get master wallet private key from .env
    let master_key = env::var("MASTER_WALLET_KEY")
        .expect("MASTER_WALLET_KEY must be set in .env file");
    let master_wallet = master_key.parse::<LocalWallet>()
        .expect("Invalid master wallet private key");

    // Get node-specific RPC URL
    let rpc_url = env::var(format!("NODE{}_RPC", node))
        .map_err(|_| eyre::eyre!("NODE{}_RPC not set in .env", node))?;
    
    // Connect to network
    let provider = Provider::<Http>::try_from(rpc_url.clone())?;
    let client = Arc::new(provider);
    let master_wallet = master_wallet.with_chain_id(client.get_chainid().await?.as_u64());

    // Read node file
    let filename = format!("node-{}.json", node);
    let file_content = fs::read_to_string(&filename)?;
    let node_data: Value = serde_json::from_str(&file_content)?;
    let senders = node_data["senders"].as_array()
        .ok_or_else(|| eyre::eyre!("No senders found in {}", filename))?;

    println!("Starting to fund Node {} sender accounts...", node);
    println!("Using RPC URL: {}", rpc_url);
    println!("Amount per account: {} ETH ({} wei)", amount_eth, amount_wei);
    let start_time = Instant::now();
    let mut total_funded = 0;

    // Get starting nonce
    let mut current_nonce = client.get_transaction_count(
        master_wallet.address(),
        None
    ).await?;

    // Fund each sender account
    for (idx, sender) in senders.iter().enumerate() {
        let address = sender["address"].as_str()
            .ok_or_else(|| eyre::eyre!("Invalid address format"))?;
        let to_address: Address = address.parse()?;

        println!("\nFunding sender account {} ({})...", idx + 1, address);

        let tx = TransactionRequest::new()
            .to(to_address)
            .value(amount_wei)
            .from(master_wallet.address())
            .gas(21_000)
            .nonce(current_nonce);

        let typed_tx = TypedTransaction::Legacy(tx);
        let signature = master_wallet.sign_transaction(&typed_tx).await?;
        let signed_tx = typed_tx.rlp_signed(&signature);
        
        match client.send_raw_transaction(signed_tx).await {
            Ok(tx_hash) => {
                println!("✓ Transaction successful!");
                println!("  Transaction hash: {}", tx_hash.tx_hash());
                total_funded += 1;
            }
            Err(e) => {
                println!("✗ Transaction failed!");
                println!("  Error: {}", e);
            }
        }

        current_nonce = current_nonce.checked_add(1.into())
            .expect("Nonce overflow");
    }

    let elapsed = start_time.elapsed();
    println!("\nFunding Summary:");
    println!("Node: {}", node);
    println!("Total accounts funded: {}", total_funded);
    println!("Amount per account: {} ETH", amount_eth);
    println!("Time taken: {:?}", elapsed);

    Ok(())
}

async fn check_node_balances(node: usize) -> eyre::Result<()> {
    // Get node-specific RPC URL
    let rpc_url = env::var(format!("NODE{}_RPC", node))
        .map_err(|_| eyre::eyre!("NODE{}_RPC not set in .env", node))?;
    
    // Connect to network
    let provider = Provider::<Http>::try_from(rpc_url.clone())?;
    let client = Arc::new(provider);

    // Read node file
    let filename = format!("node-{}.json", node);
    let file_content = fs::read_to_string(&filename)?;
    let node_data: Value = serde_json::from_str(&file_content)?;

    println!("\nChecking balances for Node {} accounts...", node);
    println!("Using RPC URL: {}", rpc_url);
    
    let chain_id = client.get_chainid().await?;
    println!("Chain ID: {}", chain_id);

    // Process both senders and receivers
    for account_type in ["senders", "receivers"] {
        let accounts = node_data[account_type].as_array()
            .ok_or_else(|| eyre::eyre!("{} not found in {}", account_type, filename))?;

        println!("\n{} Accounts:", if account_type == "senders" { "Sender" } else { "Receiver" });
        
        for (idx, account) in accounts.iter().enumerate() {
            let address = account["address"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid address format"))?;
            let address: Address = address.parse()?;

            let balance = client.get_balance(address, None).await?;
            
            println!("\nAccount {} ({}):", idx + 1, if account_type == "senders" { "Sender" } else { "Receiver" });
            println!("  Address: {}", address);
            println!("  Balance: {} wei ({} ETH)", 
                balance, 
                format_eth(balance)
            );
        }
    }

    Ok(())
}

async fn send_eth_crosschain_loop(num_nodes: usize, num_accounts: usize, amount_wei: U256, rounds: &str) -> eyre::Result<()> {
    let infinite = rounds == "#";
    let num_rounds = if infinite { 1 } else { rounds.parse::<usize>()? };
    
    // Create or open log file with new name
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("eth_transfers-Nway.log")?;  // Changed from eth_transfers.log
    let mut log = BufWriter::new(log_file);
    
    let start_time = Instant::now();
    let mut round = 1;
    let mut successful_transfers = 0;
    let mut failed_transfers = 0;
    
    loop {
        println!("\nStarting round {}", round);
        
        // Reuse existing logic but capture transactions
        let chain_ids = get_chain_ids(num_nodes).await?;
        let contract_addresses = get_contract_addresses(num_nodes).await?;
        
        for src_node in 1..=num_nodes {
            for dst_node in 1..=num_nodes {
                if src_node == dst_node {
                    continue;
                }
                
                // Get source node details
                let src_file = format!("node-{}.json", src_node);
                let src_content = fs::read_to_string(&src_file)?;
                let src_data: Value = serde_json::from_str(&src_content)?;
                
                // Get RPC URL for source node
                let rpc_url = env::var(format!("NODE{}_RPC", src_node))
                    .map_err(|_| eyre::eyre!("NODE{}_RPC not set in .env", src_node))?;
                
                let provider = Provider::<Http>::try_from(rpc_url)?;
                let client = Arc::new(provider);
                
                for acc_idx in 0..num_accounts {
                    let sender = &src_data["senders"][acc_idx];
                    let sender_key = sender["private_key"].as_str()
                        .ok_or_else(|| eyre::eyre!("Invalid private key format in {} for sender {}", 
                            src_file, acc_idx + 1))?;
                    
                    let sender_wallet = sender_key.parse::<LocalWallet>()
                        .map_err(|e| eyre::eyre!("Failed to parse sender private key in {} for account {}: {}", 
                            src_file, acc_idx + 1, e))?;
                    
                    let chain_id = client.get_chainid().await
                        .map_err(|e| eyre::eyre!("Failed to get chain ID from Node {} RPC: {}", src_node, e))?;
                    let sender_wallet = sender_wallet.with_chain_id(chain_id.as_u64());

                    // Get receiver address
                    let dst_file = format!("node-{}.json", dst_node);
                    let dst_content = fs::read_to_string(&dst_file)?;
                    let dst_data: Value = serde_json::from_str(&dst_content)?;
                    let receiver = &dst_data["receivers"][acc_idx];
                    let receiver_addr = receiver["address"].as_str()
                        .ok_or_else(|| eyre::eyre!("Invalid receiver address in {} for account {}", 
                            dst_file, acc_idx + 1))?
                        .parse::<Address>()
                        .map_err(|e| eyre::eyre!("Failed to parse receiver address: {}", e))?;

                    // Create contract instance for source node
                    println!("\nSending {} wei from Node {} (Chain ID: {}) Account {} to Node {} (Chain ID: {}) Account {}", 
                        amount_wei, src_node, chain_ids[src_node - 1], acc_idx + 1, 
                        dst_node, chain_ids[dst_node - 1], acc_idx + 1);
                    
                    println!("Using contract {} on Node {}", contract_addresses[src_node - 1], src_node);

                    let contract_json: Value = serde_json::from_slice(
                        include_bytes!("../../../reth-contract/out/MonetSmartContract.sol/MonetSmartContract.json")
                    )?;
                    let abi: ethers::abi::Abi = serde_json::from_value(contract_json["abi"].clone())?;
                    
                    let contract = Contract::new(
                        contract_addresses[src_node - 1],
                        abi,
                        Arc::new(SignerMiddleware::new(
                            client.clone(),
                            sender_wallet.clone()
                        ))
                    );

                    // Print detailed transfer information
                    println!("\nCross-chain Transfer Details:");
                    println!("  From Node {} (Chain ID: {})", src_node, chain_ids[src_node - 1]);
                    println!("  To Node {} (Chain ID: {})", dst_node, chain_ids[dst_node - 1]);
                    println!("  Amount: {} wei", amount_wei);
                    println!("  Source Account: {}", sender_wallet.address());
                    println!("  Destination Account: {}", receiver_addr);
                    println!("  Using Contract: {}", contract_addresses[src_node - 1]);

                    // Check balances before transfer
                    let sender_balance = client.get_balance(sender_wallet.address(), None).await?;
                    let gas_price = U256::zero();  // Since we're using zero gas price
                    let gas_limit = U256::from(50_000);  // Changed back to 50K
                    let total_needed = amount_wei;  // Only need to check against transfer amount since gas is free
                    
                    if sender_balance < total_needed {
                        println!("✗ Insufficient funds for cross-chain transfer!");
                        println!("  Source Chain ID: {}", chain_ids[src_node - 1]);
                        println!("  Source Address: {}", sender_wallet.address());
                        println!("  Current balance: {} wei ({} ETH)", 
                            sender_balance, format_eth(sender_balance));
                        println!("  Required balance: {} wei ({} ETH)", 
                            total_needed, format_eth(total_needed));
                        println!("  Missing: {} wei ({} ETH)", 
                            total_needed - sender_balance, format_eth(total_needed - sender_balance));
                        continue;
                    }

                    // Send transaction and log result
                    match contract.method::<_, H256>("sendETHToDestinationChain", (
                        chain_ids[dst_node - 1],
                        receiver_addr,
                    ))?.gas(gas_limit)
                      .gas_price(gas_price)
                      .value(amount_wei)
                      .send()
                      .await {
                        Ok(tx) => {
                            // Store tx_hash before await since tx will be moved
                            let tx_hash = tx.tx_hash();
                            
                            match tx.await {
                                Ok(receipt) => {
                                    if receipt.unwrap().status.unwrap().as_u64() == 1 {
                                        let tx_hash_str = format!("{:#x}", tx_hash);
                                        
                                        // Log format: tx_hash,round,timestamp,src_chain,dst_chain,from,to,amount
                                        writeln!(log, "{},{},{},{},{},{},{},{}",
                                            tx_hash_str,
                                            round,
                                            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                                            chain_ids[src_node - 1],
                                            chain_ids[dst_node - 1],
                                            sender_wallet.address(),
                                            receiver_addr,
                                            amount_wei
                                        )?;
                                        log.flush()?;
                                        
                                        println!("✓ Round {} - Transaction successful!", round);
                                        println!("  Hash: {}", tx_hash_str);
                                        successful_transfers += 1;
                                    } else {
                                        println!("✗ Round {} - Transaction failed (reverted)!", round);
                                        println!("  Hash: {:#x}", tx_hash);
                                        failed_transfers += 1;
                                    }
                                }
                                Err(e) => {
                                    println!("✗ Round {} - Transaction failed while waiting for receipt!", round);
                                    println!("  Hash: {:#x}", tx_hash);
                                    println!("  Error: {}", e);
                                    failed_transfers += 1;
                                }
                            }
                        }
                        Err(e) => {
                            println!("✗ Transaction failed: {}", e);
                            failed_transfers += 1;
                        }
                    }
                }
            }
        }
        
        if !infinite && round >= num_rounds {
            break;
        }
        round += 1;
    }
    
    let elapsed = start_time.elapsed();
    println!("\nTransfer Summary:");
    println!("Total rounds completed: {}", round);
    println!("Successful transfers: {}", successful_transfers);
    println!("Failed transfers: {}", failed_transfers);
    println!("Time taken: {:?}", elapsed);
    
    Ok(())
}

async fn get_chain_ids(num_nodes: usize) -> eyre::Result<Vec<u32>> {
    let mut chain_ids = Vec::new();
    for node_idx in 1..=num_nodes {
        let chain_id: u32 = env::var(format!("NODE{}_CHAINID", node_idx))
            .map_err(|_| eyre::eyre!("NODE{}_CHAINID not set in .env", node_idx))?
            .parse()
            .map_err(|_| eyre::eyre!("Invalid chain ID format for NODE{}_CHAINID", node_idx))?;
        println!("Debug: Node {} Chain ID: {}", node_idx, chain_id);
        chain_ids.push(chain_id);
    }
    Ok(chain_ids)
}

async fn get_contract_addresses(num_nodes: usize) -> eyre::Result<Vec<Address>> {
    let mut addresses = Vec::new();
    for node_idx in 1..=num_nodes {
        let contract_addr = env::var(format!("NODE{}_CONTRACT", node_idx))
            .map_err(|_| eyre::eyre!("NODE{}_CONTRACT not set in .env", node_idx))?
            .parse::<Address>()
            .map_err(|_| eyre::eyre!("Invalid contract address format for NODE{}_CONTRACT", node_idx))?;
        addresses.push(contract_addr);
    }
    Ok(addresses)
}