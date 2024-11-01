use hex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tiny_keccak::{Hasher, Keccak};
use std::collections::VecDeque;

const DIFFICULTY: usize = 9;
const BATCH_SIZE: u64 = 10_000_000;
const STATS_INTERVAL: Duration = Duration::from_secs(2); // 2 second interval
const MAX_RATES_BUFFER: usize = 10;

#[derive(Clone)]
struct BlockData {
    combined_data: Vec<u8>,
    nonce_position: usize,
}

impl BlockData {
    fn new(index: u64, message: &str, prev_hash: &[u8; 32], miner: &[u8; 32]) -> Self {
        let mut combined_data = Vec::with_capacity(128 + message.len());
        combined_data.extend_from_slice(&[0, 0, 0, 5]);
        combined_data.extend_from_slice(&index.to_be_bytes());
        combined_data.extend_from_slice(&[0, 0, 0, 14, 0, 0, 0, 4]);
        combined_data.extend_from_slice(message.as_bytes());
        combined_data.extend_from_slice(&[0, 0, 0, 13, 0, 0, 0, 32]);
        combined_data.extend_from_slice(prev_hash);
        combined_data.extend_from_slice(&[0, 0, 0, 5]);

        let nonce_position = combined_data.len();
        combined_data.extend_from_slice(&[0u8; 8]); // Placeholder for nonce

        combined_data.extend_from_slice(&[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
        combined_data.extend_from_slice(miner);

        BlockData {
            combined_data,
            nonce_position,
        }
    }
}

#[inline(always)]
fn check_difficulty(hash: &[u8]) -> bool {
    let first_bytes = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    first_bytes.leading_zeros() as usize >= DIFFICULTY * 4
}

fn mine_block(block_data: BlockData) -> (u64, [u8; 32]) {
    let found = Arc::new(AtomicBool::new(false));
    let hash_count = Arc::new(AtomicU64::new(0));
    let latest_nonce = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();
    let mut handles = vec![];
    let num_threads = num_cpus::get();

    // Debug thread with bounded buffer
    let found_debug = found.clone();
    let hash_count_debug = hash_count.clone();
    let latest_nonce_debug = latest_nonce.clone();
    let debug_handle = thread::spawn(move || {
        let mut last_count = 0u64;
        let mut last_time = Instant::now();
        let mut rates = VecDeque::with_capacity(MAX_RATES_BUFFER);
        
        while !found_debug.load(Ordering::Relaxed) {
            thread::sleep(STATS_INTERVAL);
            
            let current_time = Instant::now();
            let current_count = hash_count_debug.load(Ordering::Relaxed);
            let current_nonce = latest_nonce_debug.load(Ordering::Relaxed);
            let elapsed = current_time.duration_since(last_time).as_secs_f64();
            let hash_diff = current_count.saturating_sub(last_count);
            
            let rate = hash_diff as f64 / elapsed / 1_000_000.0;
            rates.push_back(rate);
            
            if rates.len() > MAX_RATES_BUFFER {
                rates.pop_front();
            }
            
            let avg_rate = rates.iter().sum::<f64>() / rates.len() as f64;
            println!("Average hashrate: {:.2} MH/s, Current nonce: {}", 
                    avg_rate, current_nonce);
            
            last_count = current_count;
            last_time = current_time;
        }
    });

    // Mining threads using pre-allocated data
    for thread_id in 0..num_threads {
        let mut block_data = block_data.clone();
        let found = found.clone();
        let hash_count = hash_count.clone();
        let latest_nonce_thread = latest_nonce.clone();
        
        handles.push(thread::spawn(move || {
            let mut hash = [0u8; 32];
            let mut local_hash_count = 0u64;
            let mut nonce = thread_id as u64;

            while !found.load(Ordering::Relaxed) {
                block_data.combined_data[block_data.nonce_position..][..8]
                    .copy_from_slice(&nonce.to_be_bytes());

                let mut hasher = Keccak::v256();
                hasher.update(&block_data.combined_data);
                hasher.finalize(&mut hash);

                if check_difficulty(&hash) {
                    found.store(true, Ordering::Release);
                    return Some((nonce, hash));
                }

                nonce += num_threads as u64;
                local_hash_count += 1;

                if local_hash_count >= BATCH_SIZE {
                    hash_count.fetch_add(local_hash_count, Ordering::Relaxed);
                    latest_nonce_thread.store(nonce, Ordering::Relaxed);
                    local_hash_count = 0;
                }
            }
            None
        }));
    }

    // Clean shutdown
    let (solution, final_hash) = handles.into_iter()
        .find_map(|h| h.join().unwrap())
        .expect("Solution should be found");

    debug_handle.join().unwrap();

    let total_hashes = hash_count.load(Ordering::Relaxed);
    let elapsed = start_time.elapsed();
    println!(
        "Found solution in {:.2}s at {:.2} MH/s",
        elapsed.as_secs_f64(),
        total_hashes as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );

    (solution, final_hash)
}

fn main() {
    let block_data = BlockData::new(
        1374,
        "KALE",
        &[0,0,0,0,231,178,107,110,214,35,200,71,158,112,48,62,115,134,14,30,144,120,252,35,159,26,108,152,80,15,34,147],
        &[71,91,242,164,88,135,40,119,138,130,113,54,158,224,57,86,17,3,255,206,53,73,64,44,224,164,121,206,191,27,9,245]
    );

    let (nonce, hash) = mine_block(block_data);
    println!("Solution nonce: {}", nonce);
    println!("Hash: {}", hex::encode(hash));
}
