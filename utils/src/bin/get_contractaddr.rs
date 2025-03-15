use ethers::{
    providers::{Provider, Http, Middleware},
    types::{TransactionReceipt, H256},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the provider (RPC URL)
    let provider = Provider::<Http>::try_from("http://34.21.80.98:8845")?;

    // Transaction hash - convert from hex string to H256
    let tx_hash = "0x897f0d3195d37c176796dd4954ceac4ffabc116c5de117853f9a8370c306ccec"
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