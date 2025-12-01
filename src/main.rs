mod utils;
mod recovery;
mod slip10;
mod network {
    pub mod evm;
    pub mod solana;
    pub mod pi;
    pub mod tron;
    pub mod doge;
}

use colored::*;
use std::io::{self, Write};
use crate::utils::{RateLimiter, print_header};
use crate::recovery::{scan_mnemonics, RecoveryConfig, TestWordInfo};
use std::sync::{Arc, Mutex};
use bip39::Language;
use regex::Regex;
use reqwest::Client;
use bitcoin::bip32::DerivationPath;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    print_header().await;
    println!("{}", "  Wallet Recovery Setup".white().bold());
    println!("{}", "  ---------------------".dimmed());
    println!("{}", "  Select Wallet Network:".cyan());
    println!("  1. EVM");
    println!("  2. Pi Network");
    println!("  3. Solana");
    println!("  4. Tron");
    println!("  5. Dogecoin");

    let network_choice = prompt(&format!("\n{}", "  > Enter Choice: ".magenta()));
    let network = match network_choice.trim() {
        "1" => "EVM",
        "2" => "PI",
        "3" => "SOLANA",
        "4" => "TRON",
        "5" => "DOGE",
        _ => {
            println!("{}", "Invalid choice. Exiting.".red());
            return;
        }
    };

    println!("{}", "\n  Select Your Wallet Seed Phrase Length:".cyan());
    println!("  1. 12 words");
    println!("  2. 15 words");
    println!("  3. 18 words");
    println!("  4. 21 words");
    println!("  5. 24 words");

    let length_choice = prompt(&format!("\n{}", "  > Enter Choice: ".magenta()));
    let mnemonic_length = match length_choice.trim() {
        "1" => 12,
        "2" => 15,
        "3" => 18,
        "4" => 21,
        "5" => 24,
        _ => {
            println!("{}", "Invalid choice. Exiting.".red());
            return;
        }
    };

    let missing_count_input = prompt(&format!("\n{}", "  How many words are missing? (1-3): ".magenta()));
    let missing_count: usize = match missing_count_input.trim().parse() {
        Ok(n) if n >= 1 && n <= 3 => n,
        _ => {
            println!("{}", "Currently only 1, 2 or 3 missing words are supported.".red());
            return;
        }
    };

    println!("{}", format!("\n  NOTE: Please enter the {} known words in sequence.", mnemonic_length - missing_count).yellow());
    println!("{}", "  Even if you are missing words, enter the known words in their correct relative order.".dimmed());
    println!("");

    let known_words: Vec<String>;
    loop {
        let known_words_input = prompt(&format!("{}", "  Known words: ".magenta()));
        let words: Vec<String> = known_words_input.trim().split_whitespace().map(|s| s.to_string()).collect();

        if words.len() != mnemonic_length - missing_count {
            println!("{}", format!("  Error: Expected {} words, received {}. Please try again.", mnemonic_length - missing_count, words.len()).red());
            continue;
        }

        let wordlist = Language::English.word_list();
        let invalid_words: Vec<String> = words.iter()
            .filter(|w| !wordlist.contains(&w.as_str()))
            .cloned()
            .collect();
        
        if !invalid_words.is_empty() {
            println!("{}", format!("  Error: Invalid BIP39 words found: {}. Please try again.", invalid_words.join(", ")).red());
            continue;
        }

        known_words = words;
        break;
    }

    let remember_position = prompt(&format!("{}", format!("  Are the positions of the {} missing word(s) known? (y/n): ", missing_count).magenta()));
    let mut known_positions = Vec::new();
    
    if is_yes(&remember_position) {
        if missing_count == 1 {
            let pos_input = prompt(&format!("{}", format!("  Enter position number (1-{}): ", mnemonic_length).magenta()));
            if let Ok(p) = pos_input.trim().parse::<usize>() {
                if p >= 1 && p <= mnemonic_length {
                    known_positions.push(p - 1);
                }
            }
        } else if missing_count == 2 {
            let p1_input = prompt(&format!("{}", format!("  Enter the position number of the missing word that comes first in the sequence (1-{}): ", mnemonic_length).magenta()));
            let p2_input = prompt(&format!("{}", format!("  Enter the position number of the missing word that comes second in the sequence (1-{}): ", mnemonic_length).magenta()));
            if let (Ok(p1), Ok(p2)) = (p1_input.trim().parse::<usize>(), p2_input.trim().parse::<usize>()) {
                if p1 >= 1 && p1 <= mnemonic_length && p2 >= 1 && p2 <= mnemonic_length && p1 != p2 {
                    known_positions.push(p1 - 1);
                    known_positions.push(p2 - 1);
                    known_positions.sort();
                }
            }
        } else {
            let p1_input = prompt(&format!("{}", format!("  Enter the position number of the missing word that comes first in the sequence (1-{}): ", mnemonic_length).magenta()));
            let p2_input = prompt(&format!("{}", format!("  Enter the position number of the missing word that comes second in the sequence (1-{}): ", mnemonic_length).magenta()));
            let p3_input = prompt(&format!("{}", format!("  Enter the position number of the missing word that comes third in the sequence (1-{}): ", mnemonic_length).magenta()));
            if let (Ok(p1), Ok(p2), Ok(p3)) = (p1_input.trim().parse::<usize>(), p2_input.trim().parse::<usize>(), p3_input.trim().parse::<usize>()) {
                let mut unique = vec![p1, p2, p3];
                unique.sort();
                unique.dedup();
                if unique.len() == 3 && unique.iter().all(|&p| p >= 1 && p <= mnemonic_length) {
                    known_positions = vec![p1 - 1, p2 - 1, p3 - 1];
                    known_positions.sort();
                }
            }
        }
    }

    let remember_address = prompt(&format!("{}", "  Is the wallet address of this seed phrase known? (y/n): ".magenta()));
    let mut target_address = None;
    let mut check_balance = false;
    let mut rpc_url = None;

    if is_yes(&remember_address) {
        let addr_input = prompt(&format!("{}", "  Enter that wallet address: ".magenta()));
        let addr = addr_input.trim().to_string();
        
        let mut is_valid_format = true;
        if network == "EVM" {
            if !Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap().is_match(&addr) {
                println!("{}", "Warning: Invalid EVM address format. Should start with 0x and be 42 characters long.".red());
                is_valid_format = false;
            }
        } else if network == "TRON" {
            if !Regex::new(r"^T[a-zA-Z0-9]{33}$").unwrap().is_match(&addr) {
                println!("{}", "Warning: Invalid Tron address format. Should start with T and be 34 characters long.".red());
                is_valid_format = false;
            }
        } else if network == "SOLANA" {
            if !Regex::new(r"^[1-9A-HJ-NP-Za-km-z]{32,44}$").unwrap().is_match(&addr) {
                println!("{}", "Warning: Invalid Solana address format. Should be Base58 encoded.".red());
                is_valid_format = false;
            }
        } else if network == "PI" {
            if !Regex::new(r"^G[A-Z0-9]{55}$").unwrap().is_match(&addr) {
                println!("{}", "Warning: Invalid Pi/Stellar address format. Should start with G and be 56 characters long.".red());
                is_valid_format = false;
            }
        } else if network == "DOGE" {
            if !Regex::new(r"^D[a-zA-Z0-9]{33}$").unwrap().is_match(&addr) {
                println!("{}", "Warning: Invalid Dogecoin address format. Should start with D and be 34 characters long.".red());
                is_valid_format = false;
            }
        }

        if !is_valid_format {
            let proceed = prompt(&format!("{}", "  Continue anyway? (y/n): ".yellow()));
            if !is_yes(&proceed) {
                return;
            }
        }
        target_address = Some(addr);
    } else {
        let has_balance = prompt(&format!("{}", "  Does the wallet contain any native gas balance? (y/n): ".magenta()));
        if is_yes(&has_balance) {
            check_balance = true;
            if network == "EVM" {
                let url_input = prompt(&format!("{}", "  Enter RPC URL for the EVM network: ".magenta()));
                let url = url_input.trim().to_string();
                if url.is_empty() {
                    println!("{}", "Error: RPC URL is required for balance checks.".red());
                    return;
                }
                rpc_url = Some(url);
            } else if network == "TRON" {
                let url_input = prompt(&format!("{}", "  Enter RPC URL for the Tron network: ".magenta()));
                let url = url_input.trim().to_string();
                if url.is_empty() {
                    println!("{}", "Error: RPC URL is required for balance checks.".red());
                    return;
                }
                rpc_url = Some(url);
            } else if network == "SOLANA" {
                let url_input = prompt(&format!("{}", "  Enter RPC URL for the Solana network: ".magenta()));
                let url = url_input.trim().to_string();
                if url.is_empty() {
                    println!("{}", "Error: RPC URL is required for balance checks.".red());
                    return;
                }
                rpc_url = Some(url);
            }
        }
    }

    if known_positions.is_empty() && target_address.is_none() && !check_balance {
        println!("{}", "\n Recovery impossible with current parameters.".red());
        println!("{}", "Without a known position, target address, or balance check, the search space is too large/complex for this tool.".yellow());
        return;
    }

    let mut positions_to_test = Vec::new();
    if !known_positions.is_empty() {
        positions_to_test.push(known_positions.clone());
    } else {
        if missing_count == 1 {
            for i in 0..mnemonic_length {
                positions_to_test.push(vec![i]);
            }
        } else if missing_count == 2 {
            for i in 0..mnemonic_length {
                for j in i + 1..mnemonic_length {
                    positions_to_test.push(vec![i, j]);
                }
            }
        } else {
            for i in 0..mnemonic_length {
                for j in i + 1..mnemonic_length {
                    for k in j + 1..mnemonic_length {
                        positions_to_test.push(vec![i, j, k]);
                    }
                }
            }
        }
    }

    let mut evm_rps = 50;
    let mut solana_rps = 50;
    let mut tron_rps = 20;

    if check_balance && (network == "EVM" || network == "SOLANA" || network == "TRON") {
        let default_rps = match network {
            "EVM" => evm_rps,
            "SOLANA" => solana_rps,
            "TRON" => tron_rps,
            _ => 0,
        };

        println!("\n  Current default rate limit is {} requests/second. {}", default_rps, "(These defaults are optimized for Alchemy free tier)".dimmed());
        println!("{}", "  Warning: Increasing this on a free tier RPC may result in rate limiting (429 errors).".yellow());

        let modify = prompt(&format!("{}", "  Do you want to modify this value? (y/n): ".magenta()));
        if is_yes(&modify) {
             let new_rps_str = prompt(&format!("{}", "  Enter desired requests per second: ".magenta()));
             if let Ok(val) = new_rps_str.trim().parse::<u32>() {
                 if val > 0 {
                     match network {
                        "EVM" => evm_rps = val,
                        "SOLANA" => solana_rps = val,
                        "TRON" => tron_rps = val,
                        _ => {},
                     }
                 }
             }
        }
    }

    let evm_limiter = Arc::new(RateLimiter::new(evm_rps, (evm_rps as usize / 2).max(1)));
    let solana_limiter = Arc::new(RateLimiter::new(solana_rps, (solana_rps as usize / 2).max(1)));
    let pi_limiter = Arc::new(RateLimiter::new(20, 10));
    let tron_limiter = Arc::new(RateLimiter::new(tron_rps, (tron_rps as usize / 2).max(1)));
    let doge_limiter = Arc::new(RateLimiter::new(3, 2));


    let client = Client::new();


    let mut paths_to_test = Vec::new();
    let mut alternative_paths = Vec::new();
    let default_path;
    
    match network {
        "EVM" => {
            default_path = network::evm::DEFAULT_PATH;
            paths_to_test.push(default_path);
            alternative_paths.extend_from_slice(network::evm::ALTERNATIVE_PATHS);
        },
        "SOLANA" => {
            default_path = network::solana::DEFAULT_PATH;
            paths_to_test.push(default_path);
            alternative_paths.extend_from_slice(network::solana::ALTERNATIVE_PATHS);
        },
        "PI" => {
            default_path = network::pi::DEFAULT_PATH;
            paths_to_test.push(default_path);
        },
        "TRON" => {
            default_path = network::tron::DEFAULT_PATH;
            paths_to_test.push(default_path);
            alternative_paths.extend_from_slice(network::tron::ALTERNATIVE_PATHS);
        },
        "DOGE" => {
            default_path = network::doge::DEFAULT_PATH;
            paths_to_test.push(default_path);
            alternative_paths.extend_from_slice(network::doge::ALTERNATIVE_PATHS);
        },
        _ => return,
    }

    let found = Arc::new(Mutex::new(false));
    let mut trying_alternatives = false;

    loop {
        let checksum_bits = mnemonic_length / 3;
        let reduction_factor = 2u64.pow(checksum_bits as u32);
        let wordlist_len = 2048u64;
        
        let mut total_to_test = 0u64;
        let last_word_pos = mnemonic_length - 1;
        
        for positions in &positions_to_test {
            let count = wordlist_len.pow(missing_count as u32);
            if positions.contains(&last_word_pos) {
                total_to_test += count / reduction_factor;
            } else {
                total_to_test += count;
            }
        }
        total_to_test *= paths_to_test.len() as u64;
        
        println!("{}", "\n\n  Configuration".white().bold());
        println!("{}", "  -------------".dimmed());
        println!("  Network:           {}", network.blue());
        println!("  Mnemonic Type:     {}", format!("{}-word", mnemonic_length).blue());
        println!("  Known Words:       {}", format!("{}/{}", known_words.len(), mnemonic_length).blue());
        println!("  Missing Words:     {}", format!("{}", missing_count).blue());
        println!("  Positions to test: {}", if !known_positions.is_empty() { format!("Known positions {}", known_positions.iter().map(|p| p+1).map(|p| p.to_string()).collect::<Vec<_>>().join(", ")) } else { "All combinations".to_string() }.blue());
        println!("  Derivation Paths:  {}", format!("{} path(s) to scan", paths_to_test.len()).blue());
        println!("  Target Address:    {}", target_address.clone().unwrap_or_else(|| "Scanning all wallets".to_string()).blue());
        if network == "EVM" || network == "TRON" || network == "SOLANA" {
            println!("  RPC URL:           {}\n", rpc_url.clone().unwrap_or_else(|| "Not provided".to_string()).blue());
        } else {
            println!("");
        }

        let start_time = std::time::Instant::now();
        let tested = Arc::new(Mutex::new(0u64));
        let print_lock = Arc::new(Mutex::new(()));

        for (path_index, current_path) in paths_to_test.iter().enumerate() {
            println!("{}", format!("\n  [Path {}/{}] Scanning path: {}", path_index + 1, paths_to_test.len(), current_path).blue());
            
            let evm_path = if network == "EVM" {
                DerivationPath::from_str(current_path).ok()
            } else {
                None
            };
            
            for positions in &positions_to_test {
                println!("{}", format!("  Scanning positions {}...", positions.iter().map(|p| p+1).map(|p| p.to_string()).collect::<Vec<_>>().join(", ")).dimmed());
                
                let config = RecoveryConfig {
                    known_words: known_words.clone(),
                    positions: positions.clone(),
                    mnemonic_length,
                };

                let found_clone = found.clone();
                let tested_clone = tested.clone();
                let target_address_clone = target_address.clone();
                let rpc_url_clone = rpc_url.clone();
                let evm_limiter_clone = evm_limiter.clone();
                let solana_limiter_clone = solana_limiter.clone();
                let pi_limiter_clone = pi_limiter.clone();
                let tron_limiter_clone = tron_limiter.clone();
                let doge_limiter_clone = doge_limiter.clone();
                let current_path_str = current_path.to_string();
                let network_str = network.to_string();
                let print_lock_clone = print_lock.clone();
                let evm_path_clone = evm_path.clone();

                let client_clone = client.clone();
                let rt_handle = tokio::runtime::Handle::current();

                tokio::task::spawn_blocking(move || {
                    scan_mnemonics(config, |mnemonic, test_words_info| {
                        if *found_clone.lock().unwrap() { return; }

                        let mut address_opt = None;
                        
                        match network_str.as_str() {
                            "EVM" => address_opt = network::evm::derive_address(&mnemonic, evm_path_clone.as_ref().expect("EVM path not parsed")),
                            "SOLANA" => address_opt = network::solana::derive_address(&mnemonic, &current_path_str),
                            "PI" => address_opt = network::pi::derive_address(&mnemonic, &current_path_str),
                            "TRON" => address_opt = network::tron::derive_address(&mnemonic, &current_path_str),
                            "DOGE" => address_opt = network::doge::derive_address(&mnemonic, &current_path_str),
                            _ => {}
                        }

                        if let Some(address) = address_opt {
                            if let Some(target) = &target_address_clone {
                                let match_found = match network_str.as_str() {
                                    "EVM" => address.to_lowercase() == target.to_lowercase(),
                                    _ => address == *target,
                                };
                                if match_found {
                                    let _lock = print_lock_clone.lock().unwrap();
                                    let display_address = if network_str == "EVM" {
                                        network::evm::to_checksum_address(&address)
                                    } else {
                                        address.clone()
                                    };
                                    print_success(&mnemonic, &display_address, &current_path_str, &test_words_info);
                                    *found_clone.lock().unwrap() = true;
                                    std::process::exit(0);
                                }
                            } else if check_balance {
                                let balance_res = rt_handle.block_on(async {
                                    let mut res = Ok(("0".to_string(), false));
                                    match network_str.as_str() {
                                        "EVM" => if let Some(url) = &rpc_url_clone { res = network::evm::check_balance(&address, url, &client_clone, &evm_limiter_clone).await; },
                                        "SOLANA" => if let Some(url) = &rpc_url_clone { res = network::solana::check_balance(&address, url, &client_clone, &solana_limiter_clone).await; },
                                        "PI" => res = network::pi::check_balance(&address, &client_clone, &pi_limiter_clone).await,
                                        "TRON" => if let Some(url) = &rpc_url_clone { res = network::tron::check_balance(&address, url, &client_clone, &tron_limiter_clone).await; },
                                        "DOGE" => res = network::doge::check_balance(&address, &client_clone, &doge_limiter_clone).await,
                                        _ => {}
                                    }
                                    res
                                });
                                
                                if let Ok((balance, has_balance)) = balance_res {
                                    if has_balance {
                                        let unit = match network_str.as_str() {
                                            "EVM" => "wei",
                                            "SOLANA" => "lamports",
                                            "PI" => "PI",
                                            "TRON" => "SUN",
                                            "DOGE" => "DOGE",
                                            _ => ""
                                        };
                                        let _lock = print_lock_clone.lock().unwrap();
                                        let display_address = if network_str == "EVM" {
                                            network::evm::to_checksum_address(&address)
                                        } else {
                                            address.clone()
                                        };
                                        print_found(&mnemonic, &display_address, &current_path_str, &test_words_info, &balance, unit);
                                    }
                                }
                            }
                        }
                    }, |delta| {
                        let mut t = tested_clone.lock().unwrap();
                        *t += delta as u64;
                        
                        let update_freq = if check_balance { 1 } else { 1000 };

                        if *t % update_freq == 0 || *t == total_to_test {
                            let now = std::time::Instant::now();
                            let elapsed_seconds = now.duration_since(start_time).as_secs_f64();
                            let elapsed = format!("{:.1}", elapsed_seconds);
                            let remaining = if total_to_test > *t { total_to_test - *t } else { 0 };
                            let progress = format!("{:.2}", (*t as f64 / total_to_test as f64) * 100.0);
                            let rate = if elapsed_seconds > 0.0 { *t as f64 / elapsed_seconds } else { 0.0 };
                            let eta = if rate > 0.0 { format!("{:.1}", remaining as f64 / rate) } else { "0.0".to_string() };
                            
                            let p_str = format!("{:>6}", progress);
                            let t_str = format!("{:>10}", t);
                            let r_str = format!("{:>10}", remaining);
                            let e_str = format!("{:>7}", elapsed);
                            let eta_str = format!("{:>7}", eta);

                            let _lock = print_lock_clone.lock().unwrap();
                            print!("\r  {}  Progress: {}% | Tested: {} | Remaining: {} | Time: {}s | ETA: {}s", 
                                "➤".yellow(), 
                                p_str.bold(), 
                                t_str.bold(), 
                                r_str.dimmed(), 
                                e_str.dimmed(), 
                                eta_str.dimmed()
                            );
                            io::stdout().flush().unwrap();
                        }
                    });
                }).await.unwrap();
                
                println!();
                
                if *found.lock().unwrap() { break; }
            }
            if *found.lock().unwrap() { break; }
        }

        if *found.lock().unwrap() { break; }

        if !trying_alternatives && !alternative_paths.is_empty() {
            if check_balance {
                println!("{}", "\n\n  Did not find your actual address yet?".yellow());
            } else {
                println!("{}", "\n\n  No exact match found with the default path.".yellow());
            }
            
            let try_alt = prompt(&format!("{}", format!("  Do you want to try {} alternative paths? (y/n): ", alternative_paths.len()).cyan()));
            if is_yes(&try_alt) {
                paths_to_test = alternative_paths.iter().map(|s| *s).collect();
                trying_alternatives = true;
                continue;
            }
        }
        break;
    }

    if !*found.lock().unwrap() && !check_balance {
        println!("{}", "\n\n  Recovery Complete: No matching wallet found within the search parameters.".red());
    } else if check_balance {
        println!("{}", "\n\n  Recovery Complete: Scanned all combinations.".green());
    }
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn is_yes(input: &str) -> bool {
    let s = input.trim().to_lowercase();
    s == "y" || s == "yes" || s == "ye" || s == "yeah" || s == "yep"
}

fn print_success(mnemonic: &str, address: &str, path: &str, info: &[TestWordInfo]) {
    println!("\n");
    println!("{}", "  ✓ RECOVERY SUCCESSFUL".green().bold());
    println!("{}", "  =====================\n".green());
    println!("  Derivation Path: {}", path.yellow());
    for item in info {
        println!("  Missing Word:    \"{}\" at position {}", item.word.yellow(), item.pos.to_string().yellow());
    }
    println!("  Address:         {}", address.green());
    println!("{}", "  ────────────────────────────────────────────────────────────────".dimmed());
    println!("  Complete Seed Phrase: {}", mnemonic.green());
    println!("{}", "  ────────────────────────────────────────────────────────────────".dimmed());
}

fn print_found(mnemonic: &str, address: &str, path: &str, info: &[TestWordInfo], balance: &str, unit: &str) {
    println!("\n");
    println!("{}", "  ✓ FOUND WALLET WITH BALANCE".green().bold());
    println!("{}", "  =========================".green());
    println!("  Address:         {}", address.green());
    println!("  Balance:         {} {}", balance.green(), unit);
    println!("  Path:            {}", path.yellow());
    for item in info {
        println!("  Missing Word:    \"{}\" at position {}", item.word.yellow(), item.pos.to_string().yellow());
    }
    println!("  Complete Seed:   {}", mnemonic.green());
    println!("{}", "  ────────────────────────────────────────────────────────────────".dimmed());
    println!("\n");
}
