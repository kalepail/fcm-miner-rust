use std::process;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tiny_keccak::{Hasher, Keccak};
use lazy_static::lazy_static;

// Increased batch size for better parallelization
const BATCH_SIZE: usize = 1_000_000;
// Use number of physical CPU cores
const LEADING_ZEROS: usize = 4;

// Lazy static for runtime CPU detection
lazy_static! {
    static ref THREAD_COUNT: usize = num_cpus::get();
}

#[inline(always)]
fn check_difficulty(hash: &[u8]) -> bool {
    hash.iter().take(LEADING_ZEROS).all(|&byte| byte == 0)
}

struct MiningData {
    prefix: Vec<u8>,
    suffix: Vec<u8>,
    counter: Arc<AtomicU64>,
    solution_found: Arc<AtomicBool>,
}

fn main() {
    let mining_data = Arc::new(MiningData {
        prefix: build_prefix(),
        suffix: build_suffix(),
        counter: Arc::new(AtomicU64::new(0)),
        solution_found: Arc::new(AtomicBool::new(false)),
    });

    let start_time = Instant::now();
    
    // Spawn worker threads and store handles
    let handles: Vec<_> = (0..*THREAD_COUNT)
        .map(|thread_id| {
            let data = Arc::clone(&mining_data);
            std::thread::Builder::new()
                .name(format!("miner-{}", thread_id))
                .spawn(move || mine_hashes(thread_id, data))
                .expect("Failed to spawn thread")
        })
        .collect();

    // Print stats until solution is found
    while !mining_data.solution_found.load(Ordering::Acquire) {
        std::thread::sleep(Duration::from_secs(1));
        let total_hashes = mining_data.counter.load(Ordering::Relaxed);
        let elapsed = start_time.elapsed().as_secs_f64();
        println!(
            "Hash rate: {:.2} MH/s, Total hashes: {}, Threads: {}",
            (total_hashes as f64) / elapsed / 1_000_000.0,
            total_hashes,
            *THREAD_COUNT
        );
    }

    // Wait for all threads to finish
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Thread join error: {:?}", e);
        }
    }

    println!("Solution found! All threads terminated.");
    process::exit(0);
}

#[inline]
fn build_prefix() -> Vec<u8> {
    let index: u64 = 1360;
    let message = "KALE";
    let prev_hash: [u8; 32] = [
        0, 0, 0, 0, 247, 76, 18, 217, 131, 35, 62, 105, 247, 183, 242, 176, 
        144, 108, 125, 103, 87, 234, 61, 129, 205, 23, 166, 149, 150, 170, 56, 165,
    ];

    let mut prefix = Vec::with_capacity(60);
    prefix.extend_from_slice(&[0, 0, 0, 5]);
    prefix.extend_from_slice(&index.to_be_bytes());
    prefix.extend_from_slice(&[0, 0, 0, 14, 0, 0, 0, 4]);
    prefix.extend_from_slice(message.as_bytes());
    prefix.extend_from_slice(&[0, 0, 0, 13, 0, 0, 0, 32]);
    prefix.extend_from_slice(&prev_hash);
    prefix.extend_from_slice(&[0, 0, 0, 5]);
    prefix
}

#[inline]
fn build_suffix() -> Vec<u8> {
    let miner: [u8; 32] = [
        71, 91, 242, 164, 88, 135, 40, 119, 138, 130, 113, 54, 158, 224, 57, 86,
        17, 3, 255, 206, 53, 73, 64, 44, 224, 164, 121, 206, 191, 27, 9, 245,
    ];

    let mut suffix = Vec::with_capacity(44);
    suffix.extend_from_slice(&[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
    suffix.extend_from_slice(&miner);
    suffix
}

fn mine_hashes(thread_id: usize, data: Arc<MiningData>) {
    // Pre-allocate reusable buffers
    let mut hasher_buffer = vec![0u8; data.prefix.len() + 8 + data.suffix.len()];
    let mut result = [0u8; 32];
    let mut nonce = u64::MAX - (thread_id as u64);

    // Copy static data into buffer
    hasher_buffer[..data.prefix.len()].copy_from_slice(&data.prefix);
    hasher_buffer[data.prefix.len() + 8..].copy_from_slice(&data.suffix);

    while !data.solution_found.load(Ordering::Acquire) {
        for _ in 0..BATCH_SIZE {
            // Update nonce in buffer
            hasher_buffer[data.prefix.len()..data.prefix.len() + 8]
                .copy_from_slice(&nonce.to_be_bytes());

            let mut hasher = Keccak::v256();
            hasher.update(&hasher_buffer);
            hasher.finalize(&mut result);

            if check_difficulty(&result) {
                println!("\nFound solution! Nonce: {}", nonce);
                println!("Hash: {:x?}", result);
                data.solution_found.store(true, Ordering::Release);
                return;
            }

            if nonce >= *THREAD_COUNT as u64 {
                nonce -= *THREAD_COUNT as u64;
            } else {
                nonce = u64::MAX - (*THREAD_COUNT as u64 - (nonce + 1));
            }
        }

        data.counter.fetch_add(BATCH_SIZE as u64, Ordering::Relaxed);
    }
}
