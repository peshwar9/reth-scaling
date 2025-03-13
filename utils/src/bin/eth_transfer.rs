use ethers::{
    core::types::TransactionRequest,
    prelude::*,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rpc_url>",args[0]);
        return Err("Invalid number of arguments".into());

    }

    let rpc_url = &args[1];

    // Set up the provider (RPC URL)
    let provider = Provider::<Http>::try_from(rpc_url)?;

    // Get the chain ID from the provider
    let chain_id = provider.get_chainid().await?;

    // Private key (replace with your actual private key)
    let wallet = "2c926ff564baeb188fe922e06de23e4ef680b7c07cf68d148fcfa5d6fa2e0f27"
        .parse::<LocalWallet>()?
        .with_chain_id(chain_id.as_u64());

    // Connect the wallet to the provider
    let client = SignerMiddleware::new(provider, wallet);

    // Create a transaction request
    let tx = TransactionRequest::new()
        .to("0x608Bf7a39D943263c28417a6Cb966E9b269bD90F") // Replace with the recipient address
        .value(U256::from_dec_str("100000000000000000000").unwrap()) // Value in wei (10 ETH)
        .gas_price(U256::from(1200000000)) // Gas price
        .gas(U256::from(21000)); // Gas limit

    // Send the transaction
    let pending_tx = client.send_transaction(tx, None).await?;
    println!("Transaction sent: {:?}", pending_tx.tx_hash());

    Ok(())
}