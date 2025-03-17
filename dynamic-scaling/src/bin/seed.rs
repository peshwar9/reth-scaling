use clap::{Parser, Subcommand};
use ethers::{
    prelude::*,
    types::{Address, U256},
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
        println!(
            "Address: {} Balance: {} ETH",
            addr,
            format_eth(*balance)
        );
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
