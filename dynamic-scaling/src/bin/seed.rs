use clap::{Parser, Subcommand};
use ethers::{
    prelude::*,
    types::{Address, TransactionRequest, transaction::eip2718::TypedTransaction, U256},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, sync::Arc};
use tokio::time::Instant;
use std::path::Path;
use dotenv::dotenv;
use std::env;

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
    /// Send ETH cross-chain between nodes
    SendEth {
        #[arg(long)]
        num_nodes: usize,
        #[arg(long)]
        num_accounts: usize,
        #[arg(long)]
        amount_wei: U256,
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
        Commands::SendEth { num_nodes, num_accounts, amount_wei } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(send_eth_crosschain(num_nodes, num_accounts, amount_wei)) {
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

async fn send_eth_crosschain(num_nodes: usize, num_accounts: usize, amount_wei: U256) -> eyre::Result<()> {
    // Validate node files and get chain IDs, contract addresses, and RPC URLs
    let mut chain_ids = Vec::new();
    let mut contract_addresses = Vec::new();
    let mut rpc_urls = Vec::new();
    
    for node_idx in 1..=num_nodes {
        // Validate node file exists
        let filename = format!("node-{}.json", node_idx);
        if !Path::new(&filename).exists() {
            return Err(eyre::eyre!("Node file {} not found", filename));
        }

        // Get chain ID, contract address, and RPC URL from environment
        let chain_id: u32 = env::var(format!("NODE{}_CHAINID", node_idx))
            .map_err(|_| eyre::eyre!("NODE{}_CHAINID not set in .env", node_idx))?
            .parse()
            .map_err(|_| eyre::eyre!("Invalid chain ID format for NODE{}_CHAINID", node_idx))?;
        
        let contract_addr = env::var(format!("NODE{}_CONTRACT", node_idx))
            .map_err(|_| eyre::eyre!("NODE{}_CONTRACT not set in .env", node_idx))?
            .parse::<Address>()
            .map_err(|_| eyre::eyre!("Invalid contract address format for NODE{}_CONTRACT", node_idx))?;

        let rpc_url = env::var(format!("NODE{}_RPC", node_idx))
            .map_err(|_| eyre::eyre!("NODE{}_RPC not set in .env", node_idx))?;

        chain_ids.push(chain_id);
        contract_addresses.push(contract_addr);
        rpc_urls.push(rpc_url);
    }

    println!("Starting cross-chain ETH transfers...");
    let start_time = Instant::now();
    let mut total_transfers = 0;
    let mut failed_transfers = 0;

    // Process transfers for each source node
    for src_node in 1..=num_nodes {
        let src_filename = format!("node-{}.json", src_node);
        let src_content = fs::read_to_string(&src_filename)
            .map_err(|e| eyre::eyre!("Failed to read source node file {}: {}", src_filename, e))?;
        let src_data: Value = serde_json::from_str(&src_content)
            .map_err(|e| eyre::eyre!("Failed to parse JSON from {}: {}", src_filename, e))?;
        
        // Connect to source node's network using its specific RPC URL
        let src_rpc = env::var(format!("NODE{}_RPC", src_node))
            .map_err(|_| eyre::eyre!("NODE{}_RPC not set in .env", src_node))?;
        let provider = Provider::<Http>::try_from(src_rpc)
            .map_err(|e| eyre::eyre!("Failed to connect to Node {} RPC: {}", src_node, e))?;
        let client = Arc::new(provider);

        // Process each sender account
        for acc_idx in 0..num_accounts {
            let sender = &src_data["senders"][acc_idx];
            let sender_key = sender["private_key"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid private key format in {} for sender {}", 
                    src_filename, acc_idx + 1))?;
            
            let sender_wallet = sender_key.parse::<LocalWallet>()
                .map_err(|e| eyre::eyre!("Failed to parse sender private key in {} for account {}: {}", 
                    src_filename, acc_idx + 1, e))?;
            
            let chain_id = client.get_chainid().await
                .map_err(|e| eyre::eyre!("Failed to get chain ID from Node {} RPC: {}", src_node, e))?;
            let sender_wallet = sender_wallet.with_chain_id(chain_id.as_u64());

            // Send to each destination node
            for dst_node in 1..=num_nodes {
                if dst_node != src_node {  // Skip self-transfers
                    let dst_filename = format!("node-{}.json", dst_node);
                    let dst_content = fs::read_to_string(&dst_filename)
                        .map_err(|e| eyre::eyre!("Failed to read destination node file {}: {}", 
                            dst_filename, e))?;
                    let dst_data: Value = serde_json::from_str(&dst_content)
                        .map_err(|e| eyre::eyre!("Failed to parse JSON from {}: {}", dst_filename, e))?;

                    // Get receiver address
                    let receiver = &dst_data["receivers"][acc_idx];
                    let receiver_addr = receiver["address"].as_str()
                        .ok_or_else(|| eyre::eyre!("Invalid receiver address in {} for account {}", 
                            dst_filename, acc_idx + 1))?
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
                    let gas_limit = U256::from(50_000);
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
                        failed_transfers += 1;
                        continue;
                    }

                    // Send transaction
                    match contract
                        .method::<_, H256>("sendETHToDestinationChain", (
                            chain_ids[dst_node - 1],
                            receiver_addr,
                        ))
                        .map_err(|e| eyre::eyre!("Failed to create contract method call: {}", e))?
                        .value(amount_wei)
                        .gas(gas_limit)
                        .gas_price(U256::zero())         // Set gas price to 0
                      //  .priority_gas_price(U256::zero())  // Set priority fee to 0
                        .send()
                        .await {
                            Ok(tx) => {
                                println!("✓ Transaction successful on chain {}!", chain_ids[src_node - 1]);
                                println!("  Transaction hash: 0x{:x}", tx.tx_hash());  // Print full hash
                                println!("  Source chain: {}", chain_ids[src_node - 1]);
                                println!("  Destination chain: {}", chain_ids[dst_node - 1]);
                                total_transfers += 1;
                            }
                            Err(e) => {
                                println!("✗ Transaction failed on chain {}!", chain_ids[src_node - 1]);
                                println!("  Source Chain ID: {}", chain_ids[src_node - 1]);
                                println!("  Source Address: {}", sender_wallet.address());
                                println!("  Current balance: {} wei ({} ETH)", 
                                    sender_balance, format_eth(sender_balance));
                                println!("  Gas limit set: {}", gas_limit);
                                println!("  Gas price: {} wei", gas_price);
                                println!("  Total gas cost: {} wei ({} ETH)", 
                                    gas_price * gas_limit, format_eth(gas_price * gas_limit));
                                println!("  Transfer amount: {} wei", amount_wei);
                                println!("  Total needed: {} wei ({} ETH)", 
                                    total_needed, format_eth(total_needed));
                                println!("  Error: {}", e);
                                failed_transfers += 1;
                            }
                        }
                }
            }
        }
    }

    let elapsed = start_time.elapsed();
    println!("\nTransfer Summary:");
    println!("Total transfers attempted: {}", total_transfers + failed_transfers);
    println!("Successful transfers: {}", total_transfers);
    println!("Failed transfers: {}", failed_transfers);
    println!("Time taken: {:?}", elapsed);

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