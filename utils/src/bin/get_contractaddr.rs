use ethers::{
    providers::{Provider, Http, Middleware},
    types::{TransactionReceipt, H256},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the provider (RPC URL)
    let provider = Provider::<Http>::try_from("http://34.48.205.25:8845")?;

    // Transaction hash - convert from hex string to H256
    let tx_hash = "0x51316f6ffe3b5d21abff33dcfcc88b4e98088a926140e29dde7d0a415693894c"
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