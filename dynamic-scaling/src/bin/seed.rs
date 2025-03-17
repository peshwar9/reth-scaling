use ethers::prelude::*;
use ethers::types::{Address, Bytes, H256, U256};
use serde_json::json;
use std::sync::Arc;
use tokio::time::Instant;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Set up Ethereum provider
    let provider = Provider::<Http>::try_from("http://127.0.0.1:8545")?;
    let client = Arc::new(provider);

    // List of 3000 addresses
    let addresses: Vec<Address> = (0..3000)
        .map(|i| Address::from_low_u64_be(i as u64))
        .collect();

    let start_time = Instant::now();

    // Call `eth_getProof` for each address
    let mut futures = Vec::new();
    for addr in &addresses {
        let params = json!([addr, []]);
        futures.push(client.request("eth_getProof", vec![params]));
    }

    let responses: Vec<serde_json::Value> = futures::future::join_all(futures).await
        .into_iter()
        .collect::<Result<_, _>>()?;

    let mut balances = Vec::new();
    for (i, response) in responses.iter().enumerate() {
        if let Some(balance) = response["balance"].as_str() {
            let balance: U256 = balance.parse()?;
            balances.push((addresses[i], balance));
        }
    }

    let elapsed = start_time.elapsed();
    println!("Fetched balances of {} accounts in {:?}", balances.len(), elapsed);

    // Print the first few balances
    for (addr, balance) in balances.iter().take(10) {
        println!("Address: {:?}, Balance: {}", addr, balance);
    }

    Ok(())
}
