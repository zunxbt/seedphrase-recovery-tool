use hmac::{Hmac, Mac};
use sha2::Sha512;

type HmacSha512 = Hmac<Sha512>;

pub fn derive_ed25519_private_key(seed: &[u8], path: &str) -> Option<[u8; 32]> {
    let mut mac = HmacSha512::new_from_slice(b"ed25519 seed").ok()?;
    mac.update(seed);
    let result = mac.finalize().into_bytes();
    let mut private_key = [0u8; 32];
    let mut chain_code = [0u8; 32];
    private_key.copy_from_slice(&result[0..32]);
    chain_code.copy_from_slice(&result[32..64]);

    let parts: Vec<&str> = path.split('/').collect();
    if parts.is_empty() || parts[0] != "m" { return None; }

    for part in &parts[1..] {
        let hardened = part.ends_with('\'');
        let index_str = if hardened { &part[0..part.len()-1] } else { part };
        let index: u32 = index_str.parse().ok()?;
        
        if !hardened {
            return None; 
        }
        

        let mut data = Vec::with_capacity(37);
        data.push(0x00);
        data.extend_from_slice(&private_key);
        data.extend_from_slice(&(0x80000000 | index).to_be_bytes());
        
        let mut mac = HmacSha512::new_from_slice(&chain_code).ok()?;
        mac.update(&data);
        let result = mac.finalize().into_bytes();
        
        private_key.copy_from_slice(&result[0..32]);
        chain_code.copy_from_slice(&result[32..64]);
    }
    
    Some(private_key)
}
