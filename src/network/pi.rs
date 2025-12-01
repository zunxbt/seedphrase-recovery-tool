
use crate::slip10::derive_ed25519_private_key;

use crate::utils::{RateLimiter, retry_with_backoff};
use ed25519_dalek::{SecretKey, PublicKey as DalekPublicKey};
use stellar_strkey::ed25519::PublicKey;
use reqwest::Client;
use serde_json::Value;

pub const DEFAULT_PATH: &str = "m/44'/314159'/0'";
pub const HORIZON_SERVER: &str = "https://api.mainnet.minepi.com";

pub fn derive_address(mnemonic_str: &str, path_str: &str) -> Option<String> {
    let mut seed_bytes = [0u8; 64];
    fastpbkdf2::pbkdf2_hmac_sha512(
        mnemonic_str.as_bytes(),
        b"mnemonic",
        2048,
        &mut seed_bytes
    );

    let private_key = derive_ed25519_private_key(&seed_bytes, path_str)?;
    
    let secret = SecretKey::from_bytes(&private_key).ok()?;
    let public: DalekPublicKey = (&secret).into();
    
    let strkey = PublicKey(public.to_bytes()).to_string();
    Some(strkey)
}

pub async fn check_balance(address: &str, client: &Client, rate_limiter: &RateLimiter) -> Result<(String, bool), String> {
    let url = format!("{}/accounts/{}", HORIZON_SERVER, address);
    
    rate_limiter.execute(async || {
        retry_with_backoff(async || {
            let res = client.get(&url).send().await.map_err(|e| e.to_string())?;
            if res.status().is_success() {
                let json: Value = res.json().await.map_err(|e| e.to_string())?;
                if let Some(balances) = json["balances"].as_array() {
                    for bal in balances {
                        if bal["asset_type"] == "native" {
                            let balance_str = bal["balance"].as_str().unwrap_or("0");
                            let balance = balance_str.parse::<f64>().unwrap_or(0.0);
                            return Ok((balance_str.to_string(), balance > 0.0));
                        }
                    }
                }
                Ok(("0".to_string(), false))
            } else if res.status().as_u16() == 404 {
                Ok(("0".to_string(), false))
            } else {
                Err(format!("Status {}", res.status()))
            }
        }, 3, 1000).await
    }).await
}
