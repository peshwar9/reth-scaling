use ethers::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let provider = Provider::<Http>::try_from("http://localhost:8545").unwrap();
    let wallet = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        .parse::<LocalWallet>()
        .unwrap();
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    let tx = TransactionRequest::pay("0x0000000000000000000000000000000000000000", 100);
    let _ = client.send_transaction(tx, None).await.unwrap();
    println!("Transaction sent");
}