use tiny_keccak::{Keccak, Hasher};

use bitcoin::bip32::{ExtendedPrivKey, DerivationPath};
use bitcoin::Network;

use crate::utils::{RateLimiter, retry_with_backoff};
use reqwest::Client;
use serde_json::json;
use lazy_static::lazy_static;
use secp256k1::{Secp256k1, All};

lazy_static! {
    static ref SECP: Secp256k1<All> = Secp256k1::new();
}

pub const DEFAULT_PATH: &str = "m/44'/60'/0'/0/0";
pub const ALTERNATIVE_PATHS: &[&str] = &[
    "m/44'/60'/0'/0/1",
    "m/44'/60'/0'/0/2",
    "m/44'/60'/0'/0/3",
    "m/44'/60'/0'/0/4",
    "m/44'/60'/0'/0/5",
    "m/44'/60'/0'/0",
    "m/44'/60'/1'/0/0",
    "m/44'/60'/2'/0/0",
];

fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut keccak = Keccak::v256();
    let mut output = [0u8; 32];
    keccak.update(data);
    keccak.finalize(&mut output);
    output
}

pub fn to_checksum_address(address: &str) -> String {
    let address = address.trim_start_matches("0x").to_lowercase();
    let hash = keccak256(address.as_bytes());
    
    let mut checksum_address = String::from("0x");
    for (i, c) in address.chars().enumerate() {
        let byte = hash[i / 2];
        let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0x0F };
        
        if nibble >= 8 {
            checksum_address.push(c.to_ascii_uppercase());
        } else {
            checksum_address.push(c);
        }
    }
    checksum_address
}

pub fn derive_address(mnemonic_str: &str, path: &DerivationPath) -> Option<String> {
    let mut seed = [0u8; 64];
    fastpbkdf2::pbkdf2_hmac_sha512(
        mnemonic_str.as_bytes(),
        b"mnemonic",
        2048,
        &mut seed
    );

    let root = ExtendedPrivKey::new_master(Network::Bitcoin, &seed).ok()?;
    let child = root.derive_priv(&SECP, path).ok()?;
    
    let public_key = secp256k1::PublicKey::from_secret_key(&SECP, &child.private_key);
    let uncompressed = public_key.serialize_uncompressed();
    
    let hash = keccak256(&uncompressed[1..]);
    let address_bytes = &hash[12..];
    let address_hex = hex::encode(address_bytes);
    
    Some(format!("0x{}", address_hex))
}

pub async fn check_balance(address: &str, rpc_url: &str, client: &Client, rate_limiter: &RateLimiter) -> Result<(String, bool), String> {
    rate_limiter.execute(async || {
        retry_with_backoff(async || {
            let body = json!({
                "jsonrpc": "2.0",
                "method": "eth_getBalance",
                "params": [address, "latest"],
                "id": 1
            });
            
            let res = client.post(rpc_url).json(&body).send().await.map_err(|e| e.to_string())?;
            if res.status().is_success() {
                let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
                if let Some(result) = json.get("result") {
                    if let Some(hex_bal) = result.as_str() {
                        let bal_str = hex_bal.trim_start_matches("0x");
                        let balance = u128::from_str_radix(bal_str, 16).unwrap_or(0);
                        return Ok((balance.to_string(), balance > 0));
                    }
                }
                Err("Invalid response".to_string())
            } else {
                Err(format!("Status {}", res.status()))
            }
        }, 3, 1000).await
    }).await
}
