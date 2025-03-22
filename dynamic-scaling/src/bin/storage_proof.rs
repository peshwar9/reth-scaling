use ethers::{
    prelude::*,
    types::{H256, U256, Bytes},
    utils::keccak256,
};
use eyre::Result;
use std::sync::Arc;
use std::env;

#[derive(Debug)]
struct CrossChainProof {
    // Transaction receipt proof
    receipt_proof: Vec<Bytes>,
    receipt_root: H256,
    tx_index: U256,
    
    // Event proof
    event_proof: Vec<Bytes>,
    event_root: H256,
    event_index: U256,
    
    // State proof for message ID
    state_proof: Vec<Bytes>,
    state_root: H256,
    message_id: U256,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Get RPC URL and contract address from env
    let rpc_url = env::var("NODE5_URL")?;
    let contract_addr = env::var("NODE5_CONTRACT")?
        .parse::<Address>()?;

    // Connect to provider
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);

    // Get transaction hash from args
    let tx_hash = env::args()
        .nth(1)
        .expect("Transaction hash required")
        .parse::<H256>()?;

    // Get transaction receipt
    let receipt = client.get_transaction_receipt(tx_hash).await?
        .expect("Transaction not found");

    // Get block
    let block = client.get_block_with_txs(receipt.block_number.unwrap())
        .await?
        .expect("Block not found");

    // Generate proofs
    let proof = generate_proof(
        &client,
        &receipt,
        &block,
        contract_addr
    ).await?;

    println!("Cross-chain proof generated:");
    println!("Receipt proof: 0x{}", hex::encode(&proof.receipt_proof));
    println!("Event proof: 0x{}", hex::encode(&proof.event_proof));
    println!("State proof: 0x{}", hex::encode(&proof.state_proof));
    println!("Message ID: {}", proof.message_id);

    Ok(())
}

async fn generate_proof(
    client: &Provider<Http>,
    receipt: &TransactionReceipt,
    block: &Block<Transaction>,
    contract_addr: Address,
) -> Result<CrossChainProof> {
    // Get receipt proof
    let receipt_proof = get_receipt_proof(client, receipt, block).await?;
    
    // Get event proof
    let event = receipt.logs.iter()
        .find(|log| log.address == contract_addr)
        .expect("Event not found");
    let event_proof = get_event_proof(client, event, receipt).await?;
    
    // Get state proof for message ID
    // The slot for messageIdByDestinationChain mapping can be calculated:
    let chain_id = event.topics[1]; // Assuming chain ID is first indexed param
    let slot = calculate_mapping_slot("messageIdByDestinationChain", chain_id);
    let state_proof = get_state_proof(client, contract_addr, slot, block.number.unwrap()).await?;

    // Get message ID from event data
    let message_id = U256::from_big_endian(&event.data.0[32..64]);

    Ok(CrossChainProof {
        receipt_proof,
        receipt_root: block.receipts_root,
        tx_index: receipt.transaction_index.unwrap_or_default().as_u64().into(),
        
        event_proof,
        event_root: H256::from_slice(&keccak256(&receipt.logs_bloom.0)),
        event_index: event.log_index.unwrap_or_default().as_u64().into(),
        
        state_proof,
        state_root: block.state_root,
        message_id,
    })
}

async fn get_receipt_proof(
    client: &Provider<Http>,
    receipt: &TransactionReceipt,
    block: &Block<Transaction>,
) -> Result<Vec<Bytes>> {
    let proof = client.get_proof(
        receipt.to.unwrap(),
        vec![H256::from_slice(&keccak256(b"receipts"))],
        Some(BlockId::Number(block.number.unwrap_or_default()))
    ).await?;
    
    Ok(proof.storage_proof[0].proof.clone())
}

async fn get_event_proof(
    client: &Provider<Http>,
    event: &Log,
    receipt: &TransactionReceipt,
) -> Result<Vec<Bytes>> {
    let proof = client.get_proof(
        event.address,
        vec![H256::from_slice(&keccak256(&event.data.0))],
        Some(BlockId::Number(receipt.block_number.unwrap_or_default()))
    ).await?;
    
    Ok(proof.storage_proof[0].proof.clone())
}

async fn get_state_proof(
    client: &Provider<Http>,
    contract: Address,
    slot: H256,
    block_number: U64,
) -> Result<Vec<Bytes>> {
    let proof = client.get_proof(
        contract,
        vec![slot],
        Some(BlockId::Number(block_number))
    ).await?;
    
    Ok(proof.storage_proof[0].proof.clone())
}

fn calculate_mapping_slot(name: &str, key: H256) -> H256 {
    // Calculate storage slot for mapping following Solidity storage layout
    let name_hash = keccak256(name.as_bytes());
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&key.0);
    data[32..].copy_from_slice(&name_hash);
    H256::from_slice(&keccak256(&data))
} 