use ethers::{
    prelude::*,
    types::{H256, Bytes, TransactionReceipt, Log, Address, EIP1186ProofResponse},
    utils::{keccak256, rlp},
    abi::AbiEncode,
};
use web3::types::Proof;
use eyre::Result;
use std::sync::Arc;
use std::env;
use hex;
use serde_json;

#[derive(Debug)]
struct CrossChainProof {
    receipt_proof: EIP1186ProofResponse,
    event_proof: EIP1186ProofResponse,
    state_proof: EIP1186ProofResponse,
    block_roots: BlockRoots,
    transaction: TransactionInfo,
}

#[derive(Debug)]
struct BlockRoots {
    state_root: H256,
    receipts_root: H256,
}

#[derive(Debug)]
struct TransactionInfo {
    receipt: TransactionReceipt,
    event: Log,
    contract_addr: Address,
    chain_id: H256,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    // Get RPC URL from .env
    dotenv::dotenv().ok();
    let rpc_url = env::var("NODE1_RPC")?;
    let provider = Provider::<Http>::try_from(rpc_url)?;
    let client = Arc::new(provider);

    // Get transaction hash from command line
    let tx_hash = env::args()
        .nth(1)
        .expect("Transaction hash required")
        .parse::<H256>()?;

    println!("Generating proof for transaction: {:?}", tx_hash);

    // Generate and verify proof
    let proof = generate_proof(&client, tx_hash).await?;
    verify_proof(&proof)?;

    Ok(())
}

async fn generate_proof(
    client: &Provider<Http>,
    tx_hash: H256,
) -> Result<CrossChainProof> {
    // First verify the chain/network
    let chain_id = client.get_chainid().await?;
    println!("\nConnected to chain ID: {}", chain_id);

    // Get the latest block to verify sync status
    let latest_block = client.get_block_number().await?;
    println!("Latest block: {}", latest_block);

    let receipt = client.get_transaction_receipt(tx_hash).await?
        .expect("Transaction not found");
    
    println!("\nTransaction receipt found:");
    println!("Block number: {:?}", receipt.block_number);
    println!("Number of logs: {}", receipt.logs.len());
    println!("To address: {:?}", receipt.to);
    println!("From address: {:?}", receipt.from);
    println!("Status: {:?}", receipt.status);
    println!("Gas used: {:?}", receipt.gas_used);

    // Get block to verify we're not querying a pruned node
    let block = client.get_block(receipt.block_number.unwrap())
        .await?
        .expect("Block not found");
    println!("\nBlock details:");
    println!("Block time: {:?}", block.timestamp);
    println!("Block hash: {:?}", block.hash);
    println!("Parent hash: {:?}", block.parent_hash);

    // Try getting raw transaction data
    let tx = client.get_transaction(tx_hash).await?
        .expect("Transaction not found");
    println!("\nRaw transaction data:");
    println!("Input length: {}", tx.input.len());
    println!("First 4 bytes (selector): 0x{}", hex::encode(&tx.input[0..4]));

    // Try direct JSON-RPC call for logs
    let block_number = format!("0x{:x}", receipt.block_number.unwrap().as_u64());
    let params = serde_json::json!([{
        "fromBlock": block_number,
        "toBlock": block_number,
        "address": format!("0x{:x}", receipt.to.unwrap())
    }]);
    
    println!("\nQuerying logs with params:");
    println!("{}", serde_json::to_string_pretty(&params)?);

    // Try multiple RPC methods
    println!("\n1. Using eth_getLogs:");
    let raw_logs = client.request::<_, serde_json::Value>("eth_getLogs", params.clone()).await?;
    println!("{}", serde_json::to_string_pretty(&raw_logs)?);

    println!("\n2. Using eth_getTransactionReceipt:");
    let raw_receipt = client.request::<_, serde_json::Value>(
        "eth_getTransactionReceipt",
        [format!("{:#x}", tx_hash)]
    ).await?;
    println!("{}", serde_json::to_string_pretty(&raw_receipt)?);

    println!("\n3. Using eth_getBlockByNumber:");
    let raw_block = client.request::<_, serde_json::Value>(
        "eth_getBlockByNumber",
        serde_json::json!([block_number, true])
    ).await?;
    println!("Block transactions: {}", 
        raw_block.get("transactions")
            .and_then(|t| t.as_array())
            .map(|t| t.len())
            .unwrap_or(0)
    );

    // Get contract address from environment
    let contract_addr = env::var("NODE1_CONTRACT")?
        .parse::<Address>()
        .expect("Invalid contract address in NODE1_CONTRACT");
    let tx_addr = receipt.to.unwrap();
    
    println!("\nContract addresses:");
    println!("Environment contract address: {:?}", contract_addr);
    println!("Transaction to address: {:?}", tx_addr);

    // Check code at both addresses
    let env_code = client.get_code(contract_addr, None).await?;
    let tx_code = client.get_code(tx_addr, None).await?;
    
    println!("\nContract code verification:");
    println!("Environment address code length: {}", env_code.len());
    println!("Transaction address code length: {}", tx_code.len());
    if !env_code.is_empty() {
        println!("Environment code starts with: 0x{}", hex::encode(&env_code[..10]));
    }
    if !tx_code.is_empty() {
        println!("Transaction code starts with: 0x{}", hex::encode(&tx_code[..10]));
    }

    // Check if addresses match
    if contract_addr != tx_addr {
        println!("\nWARNING: Transaction address doesn't match environment!");
        println!("Environment has: {:?}", contract_addr);
        println!("Transaction used: {:?}", tx_addr);
    } else {
        println!("\nTransaction address matches environment configuration.");
    }

    // Check if this is a private chain
    println!("\nChain info:");
    println!("Chain ID: {}", chain_id);
    println!("Latest block: {}", latest_block);
    println!("Block time: {}", block.timestamp);

    // Debug event signature calculation
    let event_signature_str = "ETHSentToDestinationChain(uint32,address,address,uint32,uint256)";
    let event_signature = H256::from(keccak256(event_signature_str.as_bytes()));
    println!("\nLooking for event signature:");
    println!("Event signature string: {}", event_signature_str);
    println!("Calculated signature: 0x{:x}", event_signature);

    // Decode function call
    let function_sig = "sendETHToDestinationChain(uint32,address)";
    let function_selector = &keccak256(function_sig.as_bytes())[0..4];
    println!("\nFunction details:");
    println!("Function signature: {}", function_sig);
    println!("Expected selector: 0x{}", hex::encode(function_selector));
    println!("Actual selector: 0x{}", hex::encode(&tx.input[0..4]));
    println!("Function arguments:");
    println!("  chainId (uint32): 0x{}", hex::encode(&tx.input[4..36]));
    println!("  recipient (address): 0x{}", hex::encode(&tx.input[36..68]));

    let event = receipt.logs.iter()
        .find(|log| log.topics[0] == event_signature)
        .expect("Cross-chain event not found");
    let event_clone = event.clone();  // Clone for TransactionInfo

    let contract_addr = receipt.to.unwrap();
    let chain_id = event.topics[1];

    // Generate proofs
    let receipt_proof = generate_receipt_proof(client, &receipt, block.number.unwrap().as_u64()).await?;
    let event_proof = generate_event_proof(client, event, block.number.unwrap().as_u64()).await?;
    let state_proof = generate_state_proof(
        client,
        contract_addr,
        chain_id,
        block.number.unwrap().as_u64()
    ).await?;

    Ok(CrossChainProof {
        receipt_proof,
        event_proof,
        state_proof,
        block_roots: BlockRoots {
            state_root: block.state_root,
            receipts_root: block.receipts_root,
        },
        transaction: TransactionInfo {
            receipt: receipt,
            event: event_clone,
            contract_addr,
            chain_id,
        },
    })
}

async fn generate_receipt_proof(
    client: &Provider<Http>,
    receipt: &TransactionReceipt,
    block_number: u64,
) -> Result<EIP1186ProofResponse> {
    let proof = client.get_proof(
        receipt.to.unwrap(),
        vec![H256::from(keccak256(b"receipts"))],
        Some(block_number.into())
    ).await?;

    println!("Receipt proof generated for tx index: {}", 
        receipt.transaction_index.as_u64());  // Use as_u64() directly
    Ok(proof)
}

async fn generate_event_proof(
    client: &Provider<Http>,
    event: &Log,
    block_number: u64,
) -> Result<EIP1186ProofResponse> {
    let proof = client.get_proof(
        event.address,
        vec![H256::from(keccak256(&event.data.to_vec()))],
        Some(block_number.into())
    ).await?;

    println!("Event proof generated for log index: {}", 
        event.log_index.expect("No log index").as_u64());  // Fix: unwrap Option first
    Ok(proof)
}

async fn generate_state_proof(
    client: &Provider<Http>,
    contract: Address,
    chain_id: H256,
    block_number: u64,
) -> Result<EIP1186ProofResponse> {
    let slot = calculate_mapping_slot("messageIdByDestinationChain", chain_id);
    let proof = client.get_proof(
        contract,
        vec![slot],
        Some(block_number.into())
    ).await?;
    Ok(proof)
}

fn verify_proof(proof: &CrossChainProof) -> Result<()> {
    // Convert Vec<Bytes> to &[Bytes] for verification
    let receipt_proof_slice: &[Bytes] = &proof.receipt_proof.storage_proof[0].proof;
    let event_proof_slice: &[Bytes] = &proof.event_proof.storage_proof[0].proof;
    let state_proof_slice: &[Bytes] = &proof.state_proof.storage_proof[0].proof;

    let receipt_verified = verify_merkle_proof(
        receipt_proof_slice,
        proof.block_roots.receipts_root,
        H256::from(keccak256(&rlp::encode(&proof.transaction.receipt)))
    );

    let event_verified = verify_merkle_proof(
        event_proof_slice,
        proof.block_roots.state_root,
        H256::from(keccak256(&rlp::encode(&proof.transaction.event)))
    );

    let state_verified = verify_merkle_proof(
        state_proof_slice,
        proof.block_roots.state_root,
        H256::from_slice(&proof.state_proof.storage_proof[0].value.encode())
    );

    println!("\nProof verification results:");
    println!("Receipt proof: {}", receipt_verified);
    println!("Event proof: {}", event_verified);
    println!("State proof: {}", state_verified);

    Ok(())
}

fn calculate_mapping_slot(name: &str, key: H256) -> H256 {
    let name_hash = keccak256(name.as_bytes());
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&key.0);
    data[32..].copy_from_slice(&name_hash);
    H256::from_slice(&keccak256(&data))
}

fn verify_merkle_proof(
    proof: &[Bytes],
    root: H256,
    leaf: H256
) -> bool {
    let mut current = leaf;
    for item in proof {
        current = H256::from_slice(&keccak256([&current.0, item.as_ref()].concat()));
    }
    current == root
}