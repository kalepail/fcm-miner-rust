use hex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tiny_keccak::{Hasher, Keccak};

const DIFFICULTY: usize = 9;
// const NUM_THREADS: usize = 7; // Use 7 threads to leave room for system processes
const BATCH_SIZE: u64 = 1_000_000; // Increased from 10M to 25M for better throughput
const STATS_INTERVAL: Duration = Duration::from_millis(500); // More frequent updates

#[derive(Clone)]
struct BlockData {
    static_prefix: Vec<u8>,  // Store everything before nonce
    static_suffix: Vec<u8>,  // Store everything after nonce
}

impl BlockData {
    fn new(index: u64, message: String, prev_hash: [u8; 32], miner: [u8; 32]) -> Self {
        let mut static_prefix = Vec::with_capacity(56 + message.len());
        static_prefix.extend_from_slice(&[0, 0, 0, 5]);
        static_prefix.extend_from_slice(&index.to_be_bytes());

        static_prefix.extend_from_slice(&[0, 0, 0, 14, 0, 0, 0, 4]);
        static_prefix.extend_from_slice(message.as_bytes());

        static_prefix.extend_from_slice(&[0, 0, 0, 13, 0, 0, 0, 32]);
        static_prefix.extend_from_slice(&prev_hash);

        // Nonce XDR prefix
        static_prefix.extend_from_slice(&[0, 0, 0, 5]);
        // Nonce will go here //

        let mut static_suffix = Vec::with_capacity(44);
        static_suffix.extend_from_slice(&[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
        static_suffix.extend_from_slice(&miner);

        BlockData { static_prefix, static_suffix }
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

// Main mining function with debug mode
fn mine_block(block_data: BlockData, debug_mode: bool) -> u64 {
    let found = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(Mutex::new(MiningStats::new(debug_mode)));
    let mut handles = vec![];
    let num_threads = num_cpus::get();

    for thread_id in 0..num_threads {
        let block_data = block_data.clone();
        let found = found.clone();
        let stats = stats.clone();

        handles.push(thread::spawn(move || {
            let mut hash = [0u8; 32];
            let mut nonce_bytes = [0u8; 8];
            
            let mut local_hash_count = 0u64;
            let mut nonce = thread_id as u64;

            while !found.load(Ordering::Relaxed) {
                nonce_bytes.copy_from_slice(&nonce.to_be_bytes());
                
                let mut hasher = Keccak::v256();
                hasher.update(&block_data.static_prefix);
                hasher.update(&nonce_bytes);
                hasher.update(&block_data.static_suffix);
                hasher.finalize(&mut hash);

                if check_difficulty(&hash, &nonce, debug_mode) {
                    found.store(true, Ordering::Relaxed);
                    return Some(nonce);
                }

                nonce += num_threads as u64;
                local_hash_count += 1;

                if local_hash_count >= BATCH_SIZE {
                    let mut stats = stats.lock().unwrap();
                    stats.update_hash_count(local_hash_count);
                    stats.print_stats();
                    local_hash_count = 0;
                }
            }
            None
        }));
    }

    let solution = handles.into_iter()
        .find_map(|h| h.join().unwrap())
        .expect("Solution should be found");

    let stats = stats.lock().unwrap();
    stats.print_final_stats(solution);
    
    solution
}

fn main() {
    let block_data = BlockData::new(
        1360,
        String::from("KALE"),
        [0,0,0,0,247,76,18,217,131,35,62,105,247,183,242,176,144,108,125,103,87,234,61,129,205,23,166,149,150,170,56,165],
        [71, 91, 242, 164, 88, 135, 40, 119, 138, 130, 113, 54, 158, 224, 57, 86, 17, 3, 255, 206, 53, 73, 64, 44, 224, 164, 121, 206, 191, 27, 9, 245]
    );

    let solution_nonce = mine_block(block_data, true);
    println!("Solution nonce: {}", solution_nonce);
}
