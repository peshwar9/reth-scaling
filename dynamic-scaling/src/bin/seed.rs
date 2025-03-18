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
    /// Get balances of first N accounts
    Balances {
        /// Number of accounts to check
        #[arg(short, long)]
        num_accounts: usize,
    },
    Prepare {
        #[arg(long)]
        num_accounts: usize,
        #[arg(long)]
        num_nodes: usize,
    },
    /// Fund sender accounts across all nodes
    Fund {
        #[arg(long)]
        num_nodes: usize,
        #[arg(long)]
        num_accounts: usize,
        #[arg(long)]
        amount_wei: U256,
    },
    /// Defund all accounts (senders and receivers) across all nodes back to master wallet
    Defund {
        #[arg(long)]
        num_nodes: usize,
        #[arg(long)]
        num_accounts: usize,
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
    // Load .env file at the start of the program
    dotenv().ok();
    
    let cli = Cli::parse();

    match cli.command {
        Commands::Balances { num_accounts } => {
            // Get RPC URL from environment
            let rpc_url = env::var("ETH_RPC_URL")
                .expect("ETH_RPC_URL must be set in .env file");
            check_balances(num_accounts, &rpc_url);
        }
        Commands::Prepare { num_accounts, num_nodes } => {
            prepare_node_accounts(num_accounts, num_nodes);
        }
        Commands::Fund { num_nodes, num_accounts, amount_wei } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(fund_accounts(num_nodes, num_accounts, amount_wei)) {
                eprintln!("Error funding accounts: {}", err);
            }
        }
        Commands::Defund { num_nodes, num_accounts } => {
            let runtime = tokio::runtime::Runtime::new()
                .expect("Failed to create Tokio runtime");
            
            if let Err(err) = runtime.block_on(defund_accounts(num_nodes, num_accounts)) {
                eprintln!("Error defunding accounts: {}", err);
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

async fn get_balances(num_accounts: usize, rpc_url: &str) -> eyre::Result<()> {
    // Read and parse accounts.json
    let accounts_json = fs::read_to_string("../accounts.json")?;
    let accounts: Vec<Account> = serde_json::from_str(&accounts_json)?;

    // Take only the first N accounts
    let accounts = accounts.into_iter().take(num_accounts).collect::<Vec<_>>();

    // Connect to Ethereum network
    let provider = Provider::<Http>::try_from(rpc_url.to_string())?;
    let client = Arc::new(provider);

    let start_time = Instant::now();

    // Convert addresses from string to Address type
    let addresses: Vec<Address> = accounts
        .iter()
        .map(|acc| acc.address.parse())
        .collect::<Result<_, _>>()?;

    // Call `eth_getProof` for each address
    let mut futures = Vec::new();
    for addr in &addresses {
        // Format address as proper hex string with "0x" prefix
        let params = json!([format!("0x{:x}", addr), [], "latest"]);
        futures.push(client.request("eth_getProof", params));
    }

    let responses: Vec<serde_json::Value> = futures::future::join_all(futures)
        .await
        .into_iter()
        .collect::<Result<_, _>>()?;

    // Extract balances from responses
    let mut balances = Vec::new();
    for (i, response) in responses.iter().enumerate() {
        if let Some(balance) = response["balance"].as_str() {
            let balance: U256 = balance.parse()?;
            balances.push((addresses[i], balance));
        }
    }

    let elapsed = start_time.elapsed();
    println!("Fetched balances of {} accounts in {:?}", balances.len(), elapsed);

    // Print all balances
    for (addr, balance) in balances.iter() {
        let addr_str = addr.to_string();
        let shortened = format!("{}...{}", 
            &addr_str[..6],  // First 6 chars
            &addr_str[addr_str.len()-4..]  // Last 4 chars
        );
        
        // Convert wei to ETH (1 ETH = 10^18 wei)
        let eth = balance.as_u128() as f64 / 1e18;
        
        println!("Address: {} Balance: {} wei ({:.18} ETH)", shortened, balance, eth);
    }

    Ok(())
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

fn check_balances(num_accounts: usize, rpc_url: &str) {
    // Create a tokio runtime and run the async function
    let runtime = tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime");
    
    if let Err(err) = runtime.block_on(get_balances(num_accounts, rpc_url)) {
        eprintln!("Error checking balances: {}", err);
    }
}

async fn fund_accounts(num_nodes: usize, num_accounts: usize, amount_wei: U256) -> eyre::Result<()> {
    // ... existing validation code ...

    // Get master wallet private key from .env
    let master_key = env::var("MASTER_WALLET_KEY")
        .expect("MASTER_WALLET_KEY must be set in .env file");
    let master_wallet = master_key.parse::<LocalWallet>()
        .expect("Invalid master wallet private key");

    // Connect to Ethereum network
    let rpc_url = env::var("ETH_RPC_URL")
        .expect("ETH_RPC_URL must be set in .env file");
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);
    let master_wallet = master_wallet.with_chain_id(client.get_chainid().await?.as_u64());


    // Get the current nonce for the master wallet
       let nonce = client.get_transaction_count(
        master_wallet.address(),
        None
    ).await?;
    
    let mut current_nonce = nonce;

    println!("Starting to fund accounts...");
    let start_time = Instant::now();
    let mut total_funded = 0;

    // Fund sender accounts for each node
    for node_idx in 1..=num_nodes {
        let filename = format!("node-{}.json", node_idx);
        let file_content = fs::read_to_string(&filename)?;
        let node_data: Value = serde_json::from_str(&file_content)?;
        let senders = node_data["senders"].as_array().unwrap();
        
        // Take only the first num_accounts senders
        for sender in senders.iter().take(num_accounts) {
            let address = sender["address"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid address format"))?;
            let to_address: Address = address.parse()?;

            let tx = TransactionRequest::new()
                .to(to_address)
                .value(amount_wei)
                .from(master_wallet.address())
                .gas(21_000)  // Add minimum required gas
                .nonce(current_nonce);  // Add the current nonce

            

            // Convert to TypedTransaction and sign
            let typed_tx = TypedTransaction::Legacy(tx);
            let signature = master_wallet.sign_transaction(&typed_tx).await?;
            
            // Combine transaction and signature
            let signed_tx = typed_tx.rlp_signed(&signature);
            
            // Send the transaction
            client.send_raw_transaction(signed_tx).await?;
            current_nonce = current_nonce.checked_add(1.into())
                .expect("Nonce overflow");  // Increment nonce using U256           
            total_funded += 1;
            println!("Funded account {} in node {}", address, node_idx);
        }
    }

    let elapsed = start_time.elapsed();
    println!("Successfully funded {} accounts with {} wei each in {:?}", 
        total_funded, amount_wei, elapsed);

    Ok(())
}

async fn defund_accounts(num_nodes: usize, num_accounts: usize) -> eyre::Result<()> {
    // Validate node files exist and have correct number of accounts
    for node_idx in 1..=num_nodes {
        let filename = format!("node-{}.json", node_idx);
        if !Path::new(&filename).exists() {
            return Err(eyre::eyre!("Node file {} not found", filename));
        }

        let file_content = fs::read_to_string(&filename)?;
        let node_data: Value = serde_json::from_str(&file_content)?;

        // Validate sender accounts
        let senders = node_data["senders"].as_array()
            .ok_or_else(|| eyre::eyre!("Senders not found in {}", filename))?;
        if senders.len() != num_accounts {
            return Err(eyre::eyre!("Expected {} sender accounts in {}, found {}", 
                num_accounts, filename, senders.len()));
        }

        // Validate receiver accounts
        let receivers = node_data["receivers"].as_array()
            .ok_or_else(|| eyre::eyre!("Receivers not found in {}", filename))?;
        if receivers.len() != num_accounts {
            return Err(eyre::eyre!("Expected {} receiver accounts in {}, found {}", 
                num_accounts, filename, receivers.len()));
        }
    }

    // Get master wallet address from .env
    let master_address = env::var("MASTER_WALLET_ADDRESS")
        .expect("MASTER_WALLET_ADDRESS must be set in .env file");
    let master_address: Address = master_address.parse()?;

    // Connect to Ethereum network
    let rpc_url = env::var("ETH_RPC_URL")
        .expect("ETH_RPC_URL must be set in .env file");
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);

    println!("Starting to defund accounts...");
    let start_time = Instant::now();
    let mut total_defunded = 0;

    // Helper function to process accounts
    async fn process_accounts(accounts: &[Value], node_idx: usize, master_address: Address, 
        client: &Arc<Provider<Http>>, account_type: &str) -> eyre::Result<usize> {
        let mut defunded = 0;
        
        for account in accounts {
            let private_key = account["private_key"].as_str()
                .ok_or_else(|| eyre::eyre!("Invalid private key format"))?;
            let wallet = private_key.parse::<LocalWallet>()?;
            let wallet = wallet.with_chain_id(client.get_chainid().await?.as_u64());

            // Get current balance and gas costs
            let balance = client.get_balance(wallet.address(), None).await?;
            let gas_price = client.get_gas_price().await?;
            let gas_cost = gas_price * U256::from(21_000);

            // Convert to ETH (divide by 10^18)
            let balance_eth = balance.as_u128() as f64 / 1_000_000_000_000_000_000.0;
            let gas_cost_eth = gas_cost.as_u128() as f64 / 1_000_000_000_000_000_000.0;

            println!("\nAccount {} ({})", wallet.address(), account_type);
            println!("  Current balance: {} wei ({:.6} ETH)", balance, balance_eth);
            println!("  Gas cost: {} wei ({:.6} ETH)", gas_cost, gas_cost_eth);
            println!("  Minimum balance needed: {} wei ({:.6} ETH)", gas_cost, gas_cost_eth);
            
            if balance > U256::zero() {
                if balance > gas_cost {
                    let send_amount = balance - gas_cost;
                    let send_amount_eth = send_amount.as_u128() as f64 / 1_000_000_000_000_000_000.0;
                    
                    let tx = TransactionRequest::new()
                        .to(master_address)
                        .value(send_amount)
                        .from(wallet.address())
                        .gas(21_000)
                        .gas_price(gas_price);

                    let typed_tx = TypedTransaction::Legacy(tx);
                    let signature = wallet.sign_transaction(&typed_tx).await?;
                    let signed_tx = typed_tx.rlp_signed(&signature);
                    
                    client.send_raw_transaction(signed_tx).await?;
                    
                    defunded += 1;
                    println!("  ✓ Defunded {} wei ({:.6} ETH)", send_amount, send_amount_eth);
                    println!("    Kept {} wei ({:.6} ETH) for gas", gas_cost, gas_cost_eth);
                } else {
                    let needed = gas_cost - balance;
                    let needed_eth = needed.as_u128() as f64 / 1_000_000_000_000_000_000.0;
                    println!("  ✗ Balance too low to cover gas costs");
                    println!("    Needs {} more wei ({:.6} more ETH)", needed, needed_eth);
                }
            } else {
                println!("  - No balance to defund");
            }
        }
        Ok(defunded)
    }

    // Defund both sender and receiver accounts from each node
    for node_idx in 1..=num_nodes {
        let filename = format!("node-{}.json", node_idx);
        let file_content = fs::read_to_string(&filename)?;
        let node_data: Value = serde_json::from_str(&file_content)?;
        
        // Process sender accounts
        let senders = node_data["senders"].as_array().unwrap();
        total_defunded += process_accounts(senders, node_idx, master_address, &client, "sender").await?;
        
        // Process receiver accounts
        let receivers = node_data["receivers"].as_array().unwrap();
        total_defunded += process_accounts(receivers, node_idx, master_address, &client, "receiver").await?;
    }

    let elapsed = start_time.elapsed();
    println!("Successfully defunded {} accounts in {:?}", total_defunded, elapsed);

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
                                println!("  Transaction hash: {}", tx.tx_hash());
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
    let senders = node_data["senders"].as_array()
        .ok_or_else(|| eyre::eyre!("No senders found in {}", filename))?;

    println!("\nChecking balances for Node {} sender accounts...", node);
    println!("Using RPC URL: {}", rpc_url);
    
    let chain_id = client.get_chainid().await?;
    println!("Chain ID: {}", chain_id);

    // Check balance for each sender account
    for (idx, sender) in senders.iter().enumerate() {
        let address = sender["address"].as_str()
            .ok_or_else(|| eyre::eyre!("Invalid address format"))?;
        let address: Address = address.parse()?;

        let balance = client.get_balance(address, None).await?;
        
        println!("\nSender Account {}:", idx + 1);
        println!("  Address: {}", address);
        println!("  Balance: {} wei ({} ETH)", 
            balance, 
            format_eth(balance)
        );
    }

    Ok(())
}