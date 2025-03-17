use clap::{Parser, Subcommand};
use ethers::{
    prelude::*,
    types::{Address, U256},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fs, sync::Arc};
use tokio::time::Instant;

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
}

#[derive(Debug, Serialize, Deserialize)]
struct Account {
    private_key: String,
    address: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Balances { num_accounts } => {
            get_balances(*num_accounts).await?;
        }
    }

    Ok(())
}

async fn get_balances(num_accounts: usize) -> eyre::Result<()> {
    // Read and parse accounts.json
    let accounts_json = fs::read_to_string("../accounts.json")?;
    let accounts: Vec<Account> = serde_json::from_str(&accounts_json)?;

    // Take only the first N accounts
    let accounts = accounts.into_iter().take(num_accounts).collect::<Vec<_>>();

    // Connect to Ethereum network
    let provider = Provider::<Http>::try_from(
        std::env::var("ETH_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8545".to_string()),
    )?;
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
