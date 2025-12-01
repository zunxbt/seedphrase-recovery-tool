
use bitcoin::bip32::{ExtendedPrivKey, DerivationPath};
use bitcoin::Network;
use std::str::FromStr;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;
use crate::utils::{RateLimiter, retry_with_backoff};
use reqwest::Client;
use serde_json::Value;
use lazy_static::lazy_static;
use secp256k1::{Secp256k1, All};

lazy_static! {
    static ref SECP: Secp256k1<All> = Secp256k1::new();
}

pub const DEFAULT_PATH: &str = "m/44'/3'/0'/0/0";
pub const ALTERNATIVE_PATHS: &[&str] = &[
    "m/44'/3'/0'",
    "m/44'/3'/0'/0",
    "m/44'/3'/1'/0/0",
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
    let public_key_bytes = public_key.serialize();

    let mut sha256 = Sha256::new();
    sha256.update(&public_key_bytes);
    let hash1 = sha256.finalize();

    let mut ripemd = Ripemd160::new();
    ripemd.update(hash1);
    let hash2 = ripemd.finalize();
    
    let mut address_bytes = Vec::with_capacity(25);
    address_bytes.push(0x1E);
    address_bytes.extend_from_slice(&hash2);
    
    let mut sha256_2 = Sha256::new();
    sha256_2.update(&address_bytes);
    let hash3 = sha256_2.finalize();
    
    let mut sha256_3 = Sha256::new();
    sha256_3.update(hash3);
    let hash4 = sha256_3.finalize();
    
    address_bytes.extend_from_slice(&hash4[0..4]);
    
    Some(bs58::encode(address_bytes).into_string())
}

pub async fn check_balance(address: &str, client: &Client, rate_limiter: &RateLimiter) -> Result<(String, bool), String> {
    let url = format!("https://api.blockcypher.com/v1/doge/main/addrs/{}/balance", address);
    
    rate_limiter.execute(async || {
        retry_with_backoff(async || {
            let res = client.get(&url).send().await.map_err(|e| e.to_string())?;
            if res.status().is_success() {
                let json: Value = res.json().await.map_err(|e| e.to_string())?;
                let balance_sat = json["final_balance"].as_f64().unwrap_or(0.0);
                let balance_doge = balance_sat / 100_000_000.0;
                Ok((balance_doge.to_string(), balance_sat > 0.0))
            } else if res.status().as_u16() == 429 {
                Err("rate limit".to_string())
            } else {
                Err(format!("Status {}", res.status()))
            }
        }, 3, 1000).await
    }).await
}
