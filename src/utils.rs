use colored::*;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use std::sync::Arc;

pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    min_delay: Duration,
    last_request_time: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32, max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            min_delay: Duration::from_millis(1000 / requests_per_second as u64),
            last_request_time: Arc::new(Mutex::new(Instant::now().checked_sub(Duration::from_secs(1)).unwrap_or(Instant::now()))),
        }
    }

    pub async fn execute<F, Fut, T>(&self, f: F) -> T 
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _permit = self.semaphore.acquire().await.unwrap();
        
        {
            let mut last = self.last_request_time.lock().await;
            let now = Instant::now();
            let elapsed = now.duration_since(*last);
            if elapsed < self.min_delay {
                tokio::time::sleep(self.min_delay - elapsed).await;
            }
            *last = Instant::now();
        }

        f().await
    }
}

pub async fn print_header() {
    print!("\x1B[2J\x1B[1;1H");
    
    let art1 = r#"
   ███████╗███████╗███████╗██████╗ 
   ██╔════╝██╔════╝██╔════╝██╔══██╗
   ███████╗█████╗  █████╗  ██║  ██║
   ╚════██║██╔══╝  ██╔══╝  ██║  ██║
   ███████║███████╗███████╗██████╔╝
   ╚══════╝╚══════╝╚══════╝╚═════╝ "#;
    
    let art2 = r#"
   ██████╗ ███████╗ ██████╗ ██████╗ ██╗   ██╗███████╗██████╗ ██╗   ██╗
   ██╔══██╗██╔════╝██╔════╝██╔═══██╗██║   ██║██╔════╝██╔══██╗╚██╗ ██╔╝
   ██████╔╝█████╗  ██║     ██║   ██║██║   ██║█████╗  ██████╔╝ ╚████╔╝ 
   ██╔══██╗██╔══╝  ██║     ██║   ██║╚██╗ ██╔╝██╔══╝  ██╔══██╗  ╚██╔╝  
   ██║  ██║███████╗╚██████╗╚██████╔╝ ╚████╔╝ ███████╗██║  ██║   ██║   
   ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═════╝   ╚═══╝  ╚══════╝╚═╝  ╚═╝   ╚═╝   "#;

    for line in art1.lines() {
        println!("{}", line.cyan().bold());
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    for line in art2.lines() {
        println!("{}", line.cyan().bold());
        tokio::time::sleep(Duration::from_millis(30)).await;
    }

    println!("\n");
    print!("{}", "      Built by ".white());
    use std::io::Write;
    for c in "Zun".chars() {
        print!("{}", c.to_string().magenta().bold());
        std::io::stdout().flush().unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("\n");

    let modules = vec!["EVM", "Solana", "Tron", "Pi Network", "Dogecoin"];
    for module in modules {
        print!("  Loading {} Module... ", module);
        std::io::stdout().flush().unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        println!("{}", "OK".green().bold());
        tokio::time::sleep(Duration::from_millis(40)).await;
    }

    println!("\n{}", "  ==================================================================".yellow());
    println!("\n");
}

pub async fn retry_with_backoff<F, Fut, T, E>(f: F, max_retries: u32, base_delay_ms: u64) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display + std::fmt::Debug,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                let err_msg = format!("{}", e).to_lowercase();
                let is_rate_limit = err_msg.contains("rate limit") || 
                                  err_msg.contains("429") || 
                                  err_msg.contains("503") || 
                                  err_msg.contains("too many requests");
                
                if is_rate_limit && attempt < max_retries {
                    let delay = base_delay_ms * 2u64.pow(attempt);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    attempt += 1;
                    continue;
                }
                if attempt < max_retries {
                     if err_msg.contains("econnreset") || err_msg.contains("etimedout") {
                        let delay = base_delay_ms * 2u64.pow(attempt);
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        attempt += 1;
                        continue;
                     }
                }
                return Err(e);
            }
        }
    }
}
