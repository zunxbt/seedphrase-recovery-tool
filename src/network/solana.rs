
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;

use crate::slip10::derive_ed25519_private_key;

use crate::utils::{RateLimiter, retry_with_backoff};
use ed25519_dalek::{SecretKey, PublicKey};
use reqwest::Client;
use serde_json::json;

pub const DEFAULT_PATH: &str = "m/44'/501'/0'/0'";
pub const ALTERNATIVE_PATHS: &[&str] = &[
    "m/44'/501'/0'",
    "m/44'/501'",
    "m/44'/501'/1'/0'",
    "m/44'/501'/2'/0'",
    "m/44'/501'/0'/0'/0",
];

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
    let public: PublicKey = (&secret).into();
    let keypair_dalek = ed25519_dalek::Keypair { secret, public };
    let keypair = Keypair::from_bytes(&keypair_dalek.to_bytes()).ok()?;
    
    Some(keypair.pubkey().to_string())
}

pub async fn check_balance(address: &str, rpc_url: &str, client: &Client, rate_limiter: &RateLimiter) -> Result<(String, bool), String> {
    rate_limiter.execute(async || {
        retry_with_backoff(async || {
            let body = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getBalance",
                "params": [address]
            });

            let res = client.post(rpc_url).json(&body).send().await.map_err(|e| e.to_string())?;
            if res.status().is_success() {
                let json: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
                if let Some(result) = json.get("result") {
                    let balance = result["value"].as_u64().unwrap_or(0);
                    return Ok((balance.to_string(), balance > 0));
                }
                Err("Invalid response".to_string())
            } else {
                Err(format!("Status {}", res.status()))
            }
        }, 3, 1000).await
    }).await
}
