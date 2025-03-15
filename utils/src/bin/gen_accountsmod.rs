use ethers::core::utils::secret_key_to_address;
use rand::Rng;
use clap::Parser;
use std::collections::HashMap;
use k256::SecretKey;

/// Commandline arguments

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Number of accounts to generate
    #[clap(short, long, default_value_t = 10)]
    s: usize,

    /// Number of nodes
    #[clap(short, long, default_value_t = 3)]
    n: usize
}

fn main() {
    // Parse commandline arguments

    let args = Args::parse();

    // Generate accounts and associate them with nodes
    let accounts_by_node = generate_accounts_by_node(args.s, args.n);

    // Print the accounts grouped by node
    for (node, accounts) in accounts_by_node {
        println!("Node {}:", node);
        for (private_key , address) in accounts {
            println!("  Private key: 0x{}", hex::encode(private_key));
            println!("  Address: 0x{}", address);
        }
    }
}

fn generate_accounts_by_node(s: usize, n: usize) -> HashMap<usize, Vec<([u8;32], ethers::types::Address)>>{
let mut rng = rand::thread_rng();
let mut accounts_by_node: HashMap<usize, Vec<([u8;32], ethers::types::Address)>> = HashMap::new();

for _ in 0..s {
    //generate random private key
    let private_key: [u8; 32] = rng.gen();

    // convert private key bytes to a signing key
    let secret_key = SecretKey::from_bytes(&private_key.into()).unwrap();
       // Convert SecretKey to SigningKey before passing to secret_key_to_address
       let signing_key = secret_key.into();
     
    // Derive the address from the private key
    let address = secret_key_to_address(&signing_key);

    // Extract the last 8 bytes of the address and convert to u64
    let address_bytes = address.as_bytes();
    let last_8_bytes = &address_bytes[12..20]; // last 8 bytes of 20 byte address
let address_u64 = u64::from_be_bytes(last_8_bytes.try_into().unwrap());

// Determine teh associated node
let node = (address_u64 % n as u64) as usize;

// Add account to the corresponding node list
accounts_by_node.entry(node).or_insert_with(Vec::new).push((private_key, address));
}

accounts_by_node

}