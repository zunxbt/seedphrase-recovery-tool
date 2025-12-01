use bip39::Language;
use sha2::{Sha256, Digest};
use rayon::prelude::*;


#[derive(Clone)]
pub struct TestWordInfo {
    pub pos: usize,
    pub word: String,
}

pub struct RecoveryConfig {
    pub known_words: Vec<String>,
    pub positions: Vec<usize>,
    pub mnemonic_length: usize,
}

pub fn scan_mnemonics<F, P>(
    config: RecoveryConfig,
    on_mnemonic: F,
    on_progress: P,
) 
where
    F: Fn(&str, Vec<TestWordInfo>) + Sync + Send,
    P: Fn(usize) + Sync + Send,
{
    let wordlist: Vec<&'static str> = Language::English.word_list().to_vec();
    let word_map: std::collections::HashMap<&str, u16> = wordlist
        .iter()
        .enumerate()
        .map(|(i, w)| (*w, i as u16))
        .collect();

    let mut base_indices = [0u16; 24];
    let mut known_idx = 0;
    for i in 0..config.mnemonic_length {
        if !config.positions.contains(&i) {
            if let Some(&idx) = word_map.get(config.known_words[known_idx].as_str()) {
                base_indices[i] = idx;
                known_idx += 1;
            } else {
                eprintln!("Unknown word: {}", config.known_words[known_idx]);
                return;
            }
        }
    }

    let checksum_bits = config.mnemonic_length / 3;
    let entropy_bits = (config.mnemonic_length * 11) - checksum_bits;
    let entropy_bytes = entropy_bits / 8;

    let last_word_pos = config.mnemonic_length - 1;
    let is_last_word_missing = config.positions.contains(&last_word_pos);


    let config = &config;
    let wordlist = &wordlist;
    let on_mnemonic = &on_mnemonic;
    let on_progress = &on_progress;

    if is_last_word_missing {
        let other_missing_positions: Vec<usize> = config.positions.iter().cloned().filter(|&p| p != last_word_pos).collect();
        let num_others = other_missing_positions.len();

        if num_others == 0 {
            process_inner_loop(&base_indices, config, wordlist, on_mnemonic, on_progress, last_word_pos, entropy_bytes, checksum_bits, &[]);
        } else if num_others == 1 {
            let pos0 = other_missing_positions[0];
            (0..2048).into_par_iter().for_each(|i| {
                let mut current_indices = base_indices;
                current_indices[pos0] = i;
                process_inner_loop(&current_indices, config, wordlist, on_mnemonic, on_progress, last_word_pos, entropy_bytes, checksum_bits, &[i]);
            });
        } else if num_others == 2 {
            let pos0 = other_missing_positions[0];
            let pos1 = other_missing_positions[1];
            (0..2048).into_par_iter().for_each(|i| {
                for j in 0..2048 {
                    let mut current_indices = base_indices;
                    current_indices[pos0] = i;
                    current_indices[pos1] = j;
                    process_inner_loop(&current_indices, config, wordlist, on_mnemonic, on_progress, last_word_pos, entropy_bytes, checksum_bits, &[i, j]);
                }
            });
        }
    } else {
        let num_missing = config.positions.len();
        
        if num_missing == 1 {
            let pos0 = config.positions[0];
            (0..2048).into_par_iter().for_each(|i| {
                let mut current_indices = base_indices;
                current_indices[pos0] = i;
                check_checksum(&current_indices, config, wordlist, on_mnemonic, on_progress, entropy_bytes, checksum_bits);
            });
        } else if num_missing == 2 {
            let pos0 = config.positions[0];
            let pos1 = config.positions[1];
            (0..2048).into_par_iter().for_each(|i| {
                for j in 0..2048 {
                    let mut current_indices = base_indices;
                    current_indices[pos0] = i;
                    current_indices[pos1] = j;
                    check_checksum(&current_indices, config, wordlist, on_mnemonic, on_progress, entropy_bytes, checksum_bits);
                }
            });
        } else if num_missing == 3 {
             let pos0 = config.positions[0];
             let pos1 = config.positions[1];
             let pos2 = config.positions[2];
             (0..2048).into_par_iter().for_each(|i| {
                for j in 0..2048 {
                    for k in 0..2048 {
                        let mut current_indices = base_indices;
                        current_indices[pos0] = i;
                        current_indices[pos1] = j;
                        current_indices[pos2] = k;
                        check_checksum(&current_indices, config, wordlist, on_mnemonic, on_progress, entropy_bytes, checksum_bits);
                    }
                }
            });
        }
    }
}

fn process_inner_loop<F, P>(
    base_indices: &[u16; 24],
    config: &RecoveryConfig,
    wordlist: &[&str],
    on_mnemonic: &F,
    on_progress: &P,
    last_word_pos: usize,
    entropy_bytes: usize,
    checksum_bits: usize,
    _other_indices: &[u16]
)
where
    F: Fn(&str, Vec<TestWordInfo>) + Sync + Send,
    P: Fn(usize) + Sync + Send,
{
    let missing_entropy_bits = 11 - checksum_bits;
    let max_entropy_val = 1 << missing_entropy_bits;
    
    for i in 0..max_entropy_val {
        let entropy_part = i;
        let mut bytes = [0u8; 32];
        let mut bit_ptr = 0;

        for k in 0..config.mnemonic_length - 1 {
            let idx = base_indices[k];
            for b in (0..11).rev() {
                let bit = (idx >> b) & 1;
                if bit == 1 {
                    bytes[bit_ptr / 8] |= 1 << (7 - (bit_ptr % 8));
                }
                bit_ptr += 1;
            }
        }

        for b in (0..missing_entropy_bits).rev() {
            let bit = (entropy_part >> b) & 1;
            if bit == 1 {
                bytes[bit_ptr / 8] |= 1 << (7 - (bit_ptr % 8));
            }
            bit_ptr += 1;
        }

        let mut hasher = Sha256::new();
        hasher.update(&bytes[..entropy_bytes]);
        let hash = hasher.finalize();
        let first_byte = hash[0];
        let calculated_checksum = first_byte >> (8 - checksum_bits);
        
        let last_word_index = ((i as u16) << checksum_bits) | (calculated_checksum as u16);
        
        let mut buffer = [0u8; 256];
        let mut cursor = 0;
        
        let mut final_indices = *base_indices;
        final_indices[last_word_pos] = last_word_index;
        
        for k in 0..config.mnemonic_length {
            let idx = final_indices[k];
            let word = wordlist[idx as usize];
            let word_bytes = word.as_bytes();
            let len = word_bytes.len();
            buffer[cursor..cursor+len].copy_from_slice(word_bytes);
            cursor += len;
            if k < config.mnemonic_length - 1 {
                buffer[cursor] = b' ';
                cursor += 1;
            }
        }
        
        let mnemonic_str = std::str::from_utf8(&buffer[..cursor]).unwrap();
        
        let mut test_words_info = Vec::new();
        for &pos in &config.positions {
            test_words_info.push(TestWordInfo {
                pos: pos + 1,
                word: wordlist[final_indices[pos] as usize].to_string(),
            });
        }
        
        on_mnemonic(mnemonic_str, test_words_info);
        on_progress(1);
    }
}

fn check_checksum<F, P>(
    indices: &[u16; 24],
    config: &RecoveryConfig,
    wordlist: &[&str],
    on_mnemonic: &F,
    on_progress: &P,
    entropy_bytes: usize,
    checksum_bits: usize
)
where
    F: Fn(&str, Vec<TestWordInfo>) + Sync + Send,
    P: Fn(usize) + Sync + Send,
{
    let mut bytes = [0u8; 32];
    let mut bit_ptr = 0;
    let entropy_bits_count = entropy_bytes * 8;

    for i in 0..config.mnemonic_length {
        let idx = indices[i];
        for b in (0..11).rev() {
            let bit = (idx >> b) & 1;
            if bit_ptr < entropy_bits_count {
                if bit == 1 {
                    bytes[bit_ptr / 8] |= 1 << (7 - (bit_ptr % 8));
                }
            }
            bit_ptr += 1;
        }
    }
    
    let mut hasher = Sha256::new();
    hasher.update(&bytes[..entropy_bytes]);
    let hash = hasher.finalize();
    let first_byte = hash[0];
    let calculated_checksum = first_byte >> (8 - checksum_bits);
    
    let last_word = indices[config.mnemonic_length - 1];
    let actual_checksum = (last_word & ((1 << checksum_bits) - 1)) as u8;
    
    if calculated_checksum == actual_checksum {
        let mut buffer = [0u8; 256];
        let mut cursor = 0;
        
        for k in 0..config.mnemonic_length {
            let idx = indices[k];
            let word = wordlist[idx as usize];
            let word_bytes = word.as_bytes();
            let len = word_bytes.len();
            buffer[cursor..cursor+len].copy_from_slice(word_bytes);
            cursor += len;
            if k < config.mnemonic_length - 1 {
                buffer[cursor] = b' ';
                cursor += 1;
            }
        }
        
        let mnemonic_str = std::str::from_utf8(&buffer[..cursor]).unwrap();
        
        let mut test_words_info = Vec::new();
        for &pos in &config.positions {
            test_words_info.push(TestWordInfo {
                pos: pos + 1,
                word: wordlist[indices[pos] as usize].to_string(),
            });
        }
        
        on_mnemonic(mnemonic_str, test_words_info);
    }
    on_progress(1);
}
