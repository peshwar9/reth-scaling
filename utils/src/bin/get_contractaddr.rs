use ethers::{
    providers::{Provider, Http, Middleware},
    types::{TransactionReceipt, H256},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the provider (RPC URL)
    let provider = Provider::<Http>::try_from("http://34.21.80.98:8845")?;

    // Transaction hash - convert from hex string to H256
    let tx_hash = "0x5866b5c382d7e7670fd59c7b3786ff5ab73cdce02a9910629dbe2ba9dadf7c62"
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