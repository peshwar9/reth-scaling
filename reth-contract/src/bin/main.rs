use ethers::{
    core::utils::hex,
    prelude::*,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the provider (RPC URL)
    let provider = Provider::<Http>::try_from("http://128.199.25.233:22001")?;

    // Get and print chain ID
    let chain_id = provider.get_chainid().await?;
    println!("Connected to chain ID: {}", chain_id);

    // Private key (replace with your actual private key)
    let wallet = "4efafbca52c7fe393a1867dc2082acc9a6a3f96dd47ab2b106954e7227872ea7"
        .parse::<LocalWallet>()?
        .with_chain_id(chain_id.as_u64());

    // Print wallet address for debugging
    println!("Wallet address: {}", wallet.address());

    // Get balance to verify account
    let balance = provider.get_balance(wallet.address(), None).await?;
    println!("Wallet balance: {} ETH", balance.as_u128() as f64 / 1e18);

    // Connect the wallet to the provider
    let client = SignerMiddleware::new(provider, wallet);

    // Read bytecode from file and parse JSON
    let bytecode_str = std::fs::read_to_string("bytecode.txt")?;
    
    // Parse JSON and extract bytecode
    let json: serde_json::Value = serde_json::from_str(&bytecode_str)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    let bytecode_str = json.get("object")
        .and_then(|b| b.as_str())
        .ok_or("No 'object' field found in JSON")?
        .trim_start_matches("0x");

    // Decode the hex string
    let bytecode = hex::decode(bytecode_str)?;

    // Create a transaction request
    let tx = TransactionRequest::new()
        .data(bytecode)
        .gas(U256::from(6000000)); // Gas limit

    // Send the transaction
    let pending_tx = client.send_transaction(tx, None).await?;
    println!("Transaction sent: {:?}", pending_tx.tx_hash());

    Ok(())
}