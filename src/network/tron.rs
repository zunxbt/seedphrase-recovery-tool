
use bitcoin::bip32::{ExtendedPrivKey, DerivationPath};
use bitcoin::Network;
use std::str::FromStr;
use sha2::{Sha256, Digest};
use tiny_keccak::{Keccak, Hasher};
use crate::utils::{RateLimiter, retry_with_backoff};
use reqwest::Client;
use serde_json::json;
use lazy_static::lazy_static;
use secp256k1::{Secp256k1, All};

lazy_static! {
    static ref SECP: Secp256k1<All> = Secp256k1::new();
}

pub const DEFAULT_PATH: &str = "m/44'/195'/0'/0/0";
pub const ALTERNATIVE_PATHS: &[&str] = &[
    "m/44'/60'/0'/0",
    "m/44'/195'/0'/0/1",
    "m/44'/195'/0'/0/2",
];

pub fn derive_address(mnemonic_str: &str, path_str: &str) -> Option<String> {
    let mut seed_bytes = [0u8; 64];
    fastpbkdf2::pbkdf2_hmac_sha512(
        mnemonic_str.as_bytes(),
        b"mnemonic",
        2048,
        &mut seed_bytes
    );

    let root = ExtendedPrivKey::new_master(Network::Bitcoin, &seed_bytes).ok()?;
    let path = DerivationPath::from_str(path_str).ok()?;
    let child = root.derive_priv(&SECP, &path).ok()?;
    
    let public_key = secp256k1::PublicKey::from_secret_key(&SECP, &child.private_key);
    let uncompressed = public_key.serialize_uncompressed();
    
    let mut keccak = Keccak::v256();
    let mut output = [0u8; 32];
    keccak.update(&uncompressed[1..]);
    keccak.finalize(&mut output);
    
    let mut address_bytes = Vec::with_capacity(25);
    address_bytes.push(0x41);
    address_bytes.extend_from_slice(&output[12..]);
    
    let mut hasher = Sha256::new();
    hasher.update(&address_bytes);
    let hash1 = hasher.finalize();
    
    let mut hasher2 = Sha256::new();
    hasher2.update(hash1);
    let hash2 = hasher2.finalize();
    
    let checksum = &hash2[0..4];
    address_bytes.extend_from_slice(checksum);
    
    Some(bs58::encode(address_bytes).into_string())
}

pub async fn check_balance(address: &str, rpc_url: &str, client: &Client, rate_limiter: &RateLimiter) -> Result<(String, bool), String> {
    let decoded = bs58::decode(address).into_vec().map_err(|e| e.to_string())?;
    if decoded.len() < 4 { return Err("Invalid address length".to_string()); }
    let hex_address = hex::encode(&decoded[0..decoded.len()-4]);
    let hex_address_prefixed = format!("0x{}", hex_address);

    rate_limiter.execute(async || {
        retry_with_backoff(async || {
            let body = json!({
                "jsonrpc": "2.0",
                "method": "eth_getBalance",
                "params": [hex_address_prefixed, "latest"],
                "id": 1
            });
            
            let res = client.post(rpc_url).json(&body).send().await.map_err(|e| e.to_string())?;
            if res.status().is_success() {
                let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
                if let Some(result) = json.get("result") {
                    if let Some(hex_bal) = result.as_str() {
                        let bal_str = hex_bal.trim_start_matches("0x");
                        let balance_sun = u128::from_str_radix(bal_str, 16).unwrap_or(0);
                        let balance_trx = balance_sun as f64 / 1_000_000.0;
                        return Ok((balance_trx.to_string(), balance_sun > 0));
                    }
                }
                Err("Invalid response".to_string())
            } else {
                Err(format!("Status {}", res.status()))
            }
        }, 3, 1000).await
    }).await
}
