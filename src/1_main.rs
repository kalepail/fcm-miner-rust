use hex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tiny_keccak::{Hasher, Keccak};

const DIFFICULTY: usize = 4;
const BATCH_SIZE: u64 = 1_000_000;
const THREAD_COUNT: usize = 10; // Adjust based on CPU cores
const STATS_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
struct BlockData {
    prefix: Vec<u8>,
    suffix: Vec<u8>,
}

impl BlockData {
    #[inline(always)]
    fn new(index: u64, message: &str, prev_hash: &[u8; 32], miner: &[u8; 32]) -> Self {
        let mut prefix = Vec::with_capacity(56 + message.len());
        prefix.extend_from_slice(&[0, 0, 0, 5]);
        prefix.extend_from_slice(&index.to_be_bytes());
        prefix.extend_from_slice(&[0, 0, 0, 14, 0, 0, 0, 4]);
        prefix.extend_from_slice(message.as_bytes());
        prefix.extend_from_slice(&[0, 0, 0, 13, 0, 0, 0, 32]);
        prefix.extend_from_slice(prev_hash);
        prefix.extend_from_slice(&[0, 0, 0, 5]);

        let mut suffix = Vec::with_capacity(44);
        suffix.extend_from_slice(&[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
        suffix.extend_from_slice(miner);

        BlockData { prefix, suffix }
    }
}

// New statistics collection struct
struct MiningStats {
    start_time: Instant,
    hash_count: Arc<AtomicU64>,
    debug_mode: bool,
    last_hash_count: u64,
    last_update: Instant,
}

impl MiningStats {
    fn new(debug_mode: bool) -> Self {
        Self {
            start_time: Instant::now(),
            hash_count: Arc::new(AtomicU64::new(0)),
            debug_mode,
            last_hash_count: 0,
            last_update: Instant::now(),
        }
    }

    fn update_hash_count(&mut self, increment: u64) {
        self.hash_count.fetch_add(increment, Ordering::Relaxed);
    }

    fn print_stats(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        
        if elapsed >= STATS_INTERVAL {
            let current_count = self.hash_count.load(Ordering::Relaxed);
            let hashes = current_count - self.last_hash_count;
            let hash_rate = hashes as f64 / elapsed.as_secs_f64();

            if self.debug_mode {
                println!("Hash rate: {:.2} MH/s", hash_rate / 1_000_000.0);
            }

            self.last_hash_count = current_count;
            self.last_update = now;
        }
    }

    fn print_final_stats(&self, solution_nonce: u64) {
        let total_time = self.start_time.elapsed();
        let total_hashes = self.hash_count.load(Ordering::Relaxed);
        let avg_hash_rate = total_hashes as f64 / total_time.as_secs_f64();

        println!("\n=== Mining Complete ===");
        println!("Total time: {:.2} seconds", total_time.as_secs_f64());
        println!("Total hashes: {}", total_hashes);
        println!("Average hash rate: {:.2} MH/s", avg_hash_rate / 1_000_000.0);
        println!("Solution nonce: {}", solution_nonce);
    }
}

// Modified check_difficulty function
#[inline(always)]
fn check_difficulty(hash: &[u8], nonce: &u64, debug_mode: bool) -> bool {
    let first_bytes = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    let leading_zeros = first_bytes.leading_zeros() as usize;

    if leading_zeros >= DIFFICULTY * 4 {
        if debug_mode {
            println!("\nFound hash with {} leading zeros!", DIFFICULTY);
            println!("Hash (hex): {:016x}", first_bytes);
            println!("Full hash: {}", hex::encode(hash));
            println!("Nonce: {}\n", nonce);
        }
        true
    } else {
        false
    }
}

#[inline(always)]
fn mine_nonce(data: &BlockData, start_nonce: u64, end_nonce: u64) -> Option<(u64, [u8; 32])> {
    let mut buffer = Vec::with_capacity(data.prefix.len() + 8 + data.suffix.len());
    let mut hasher = Keccak::v256();
    let mut hash = [0u8; 32];
    
    buffer.extend_from_slice(&data.prefix);
    let nonce_start = buffer.len();
    buffer.extend_from_slice(&[0u8; 8]);
    buffer.extend_from_slice(&data.suffix);

    for nonce in start_nonce..end_nonce {
        buffer[nonce_start..nonce_start + 8].copy_from_slice(&nonce.to_be_bytes());
        hasher = Keccak::v256();
        hasher.update(&buffer);
        hasher.finalize(&mut hash);
        
        if hash.iter().take(DIFFICULTY).all(|&x| x == 0) {
            return Some((nonce, hash));
        }
    }
    None
}

pub fn mine_parallel(data: BlockData) -> (u64, [u8; 32]) {
    let found = Arc::new(AtomicBool::new(false));
    let handles: Vec<_> = (0..THREAD_COUNT)
        .map(|thread_id| {
            let data = data.clone();
            let found = found.clone();
            thread::spawn(move || {
                let mut nonce = thread_id as u64;
                while !found.load(Ordering::Relaxed) {
                    if let Some(result) = mine_nonce(&data, nonce, nonce + BATCH_SIZE) {
                        found.store(true, Ordering::Relaxed);
                        return Some(result);
                    }
                    nonce += THREAD_COUNT as u64 * BATCH_SIZE;
                }
                None
            })
        })
        .collect();

    for handle in handles {
        if let Some(result) = handle.join().unwrap() {
            return result;
        }
    }
    unreachable!()
}

fn main() {
    let block_data = BlockData::new(
        1352,
        "KALE",
        &[0,0,0,0,195,158,128,225,94,30,123,14,226,80,172,214,235,76,32,83,213,25,217,215,30,170,202,1,152,127,0,7],
        &[71,91,242,164,88,135,40,119,138,130,113,54,158,224,57,86,17,3,255,206,53,73,64,44,224,164,121,206,191,27,9,245]
    );

    println!("Starting mining...");
    let start_time = std::time::Instant::now();
    
    let (nonce, hash) = mine_parallel(block_data);
    
    let duration = start_time.elapsed();
    println!("\nMining completed in {:.2?}", duration);
    println!("Found nonce: {}", nonce);
    println!("Hash: {}", hex::encode(hash));
}
