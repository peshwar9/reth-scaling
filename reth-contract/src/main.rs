use ethers::{
    abi::Abi,
    contract::ContractFactory,
    core::types::U256,
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::TransactionRequest,
};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Load private key from environment variable
    let private_key = env::var("PRIVATE_KEY")
        .expect("Set PRIVATE_KEY environment variable with your Ethereum wallet private key");

    // Connect to the Reth PoA instance (Assumes Reth is running on localhost:8545)
    let provider = Provider::<Http>::try_from("http://localhost:8545")?;
    
    // Load wallet from private key
    let wallet = private_key.parse::<LocalWallet>()?.with_chain_id(1337); // Match your PoA chain ID

    // Use SignerMiddleware to sign transactions
    let client = Arc::new(SignerMiddleware::new(provider, wallet.clone()));

    // ABI and Bytecode (Replace with your actual contract ABI and bytecode)
    let abi: Abi = serde_json::from_str(include_str!("SimpleStorage.abi.json"))?;
    let bytecode = include_str!("SimpleStorage.bin"); // Raw bytecode as hex string
    let bytecode = hex::decode(bytecode.trim_start_matches("0x"))?;

    // Deploy the contract
    let factory = ContractFactory::new(abi, bytecode.into(), client.clone());
    let deployer = factory.deploy(U256::from(42))?; // Example constructor param
    let contract = deployer.send().await?;

    println!("Contract deployed at: {:?}", contract.address());
    Ok(())
}
