use ethers::{
    providers::{Provider, Http, Middleware},
    types::{TransactionReceipt, H256},
};
use std::env;
use dotenv::dotenv;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    
    // Set up the provider using NODE1_RPC from env
    let rpc_url = env::var("NODE1_RPC")
        .expect("RPC_URL must be set in .env file");
    let provider = Provider::<Http>::try_from(rpc_url)?;

    // Transaction hash - convert from hex string to H256
    let tx_hash = "0x3f2e6666c282713565fc41b679358dad74dbabb64fb006aed6ce1be9194b56e9"
        .parse::<H256>()?;

    // Get the transaction receipt
    let receipt: Option<TransactionReceipt> = provider.get_transaction_receipt(tx_hash).await?;

    if let Some(receipt) = receipt {
        if let Some(contract_address) = receipt.contract_address {
            println!("Contract address: {:?}", contract_address);
        } else {
            println!("This transaction is not a contract deployment.");
        }
    } else {
        println!("Transaction receipt not found.");
    }

    Ok(())
}