use hex;

fn decode_rpc_url(hex_str: &str) -> String {
    let clean_hex = hex_str.trim_start_matches("0x"); // Remove "0x" prefix
    let bytes = hex::decode(clean_hex).expect("Invalid hex string");
    let decoded_str = String::from_utf8_lossy(&bytes);
    decoded_str.trim_matches(char::from(0)).to_string() // Trim null bytes
}

fn main() {
    let hex_data = "0x3838343500000000000000000000000000000000"; // Example input
    
    let decoded_url = decode_rpc_url(hex_data);
    println!("Decoded RPC URL: {}", decoded_url);
}
