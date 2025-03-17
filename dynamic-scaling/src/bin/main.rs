use std::sync::Arc;
use tokio::sync::Mutex;
use web3::types::{Address, U256};
use web3::Web3;
use web3::transports::Http;
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone)]
struct Chain {
    rpc_url: String,
    treasury_account: Address,
    accounts: Vec<Address>,
}

#[tokio::main]
async fn main() {
    // Configuration
    let chains = vec![
        Chain {
            rpc_url: "http://chain-a-rpc-url".to_string(),
            treasury_account: Address::from_str("0xTreasuryA").unwrap(),
            accounts: vec![],
        },
        Chain {
            rpc_url: "http://chain-b-rpc-url".to_string(),
            treasury_account: Address::from_str("0xTreasuryB").unwrap(),
            accounts: vec![],
        },
        Chain {
            rpc_url: "http://chain-c-rpc-url".to_string(),
            treasury_account: Address::from_str("0xTreasuryC").unwrap(),
            accounts: vec![],
        },
    ];

    let num_accounts = 300;
    let initial_balance = U256::from(10_000_000_000_000_000u64); // 0.01 ETH in wei

    // Initialize chains and accounts
    let chains = Arc::new(Mutex::new(chains));
    let chains_clone = Arc::clone(&chains);

    // Generate accounts and seed them with initial balance
    for i in 0..chains_clone.lock().await.len() {
        let chain = &mut chains_clone.lock().await[i];
        let web3 = Web3::new(Http::new(&chain.rpc_url).unwrap());

        for _ in 0..num_accounts {
            let account = Address::random();
            chain.accounts.push(account);

            // Transfer 0.01 ETH from treasury to the new account
            let tx = web3.eth().send_transaction(web3::types::TransactionRequest {
                from: chain.treasury_account,
                to: Some(account),
                value: Some(initial_balance),
                ..Default::default()
            });

            if let Err(e) = tx.await {
                eprintln!("Failed to seed account: {:?}", e);
            }
        }
    }

    // Start sending 1 wei between chains
    let chains_clone = Arc::clone(&chains);
    let chain_len = chains_clone.lock().await.len();
    let handles: Vec<_> = (0..chain_len).map(|i| {
        let chains = Arc::clone(&chains_clone);
        let chains_for_data = Arc::clone(&chains_clone);  // Create a new clone for the async block
        let chain_data = async move {
            let locked_chains = chains_for_data.lock().await;
            (locked_chains[i].rpc_url.clone(), locked_chains[i].treasury_account)
        };
        let (chain_rpc, treasury) = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(chain_data);
        
        tokio::spawn(async move {
            let web3 = Web3::new(Http::new(&chain_rpc).unwrap());
            loop {
                let chains = chains.lock().await;
                for (j, other_chain) in chains.iter().enumerate() {
                    if i != j {
                        for account in &other_chain.accounts {
                            let tx = web3.eth().send_transaction(web3::types::TransactionRequest {
                                from: treasury,
                                to: Some(*account),
                                value: Some(U256::from(1)), // 1 wei
                                ..Default::default()
                            });

                            if let Err(e) = tx.await {
                                eprintln!("Failed to send 1 wei: {:?}", e);
                            }
                        }
                    }
                }
                drop(chains); // explicitly release the lock
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    }).collect();

    // Wait for all tasks to complete (they won't, since they loop forever)
    for handle in handles {
        handle.await.unwrap();
    }
}