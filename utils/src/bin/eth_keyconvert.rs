use secp256k1::{Secp256k1, SecretKey};
use ethereum_types::H160;
use hex;
use std::env;
use tiny_keccak::{Keccak, Hasher};

fn main() {
    // Read the private key from commandline
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <private_key>",args[0]);
        return;
    }

    let private_key_hex = &args[1];

    //Decode the private key from hex
    let private_key_bytes = match hex::decode(private_key_hex.strip_prefix("0x").unwrap_or(private_key_hex)) {
        Ok(bytes) => bytes,
        Err(_) => {
            eprintln!("Invalid private key: must be a hex string with or without a prefix");    
            return;
        }
    };
    // Ensure the private key is 32 bytes long
    if private_key_bytes.len() != 32 {
        eprintln!("Invalid private key, should be atleast 32 bytes long");
        return;
    }

    //create Secp256k1 context
    let secp = Secp256k1::new();

    // parse the private key
    let secret_key = match SecretKey::from_slice(&private_key_bytes) {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Invalid privatekey: must be a valid secp256k1 private key");
            return;
        }
    };

    let public_key = secp256k1::PublicKey::from_secret_key(&secp,&secret_key);
    // Serialize the public key in uncompressed format (65 bytes)
    let public_key_uncompressed = public_key.serialize_uncompressed();

    // Derive the Ethereum address
    // Hash is the last 20 bytes of the keccak-256 hash of public key
    let mut hasher = Keccak::v256();
    hasher.update(&public_key_uncompressed[1..]); // Skip th efirst byte compression flag

    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);

    let address = H160::from_slice(&hash[12..]); // take the last 20 bytes
//Output results
    println!("Private Key: 0x{}", hex::encode(&private_key_bytes));
    println!("Public Key: 0x{}", hex::encode(public_key_uncompressed));
    println!("Ethereum Address: 0x{}", hex::encode(address.as_bytes()));
}