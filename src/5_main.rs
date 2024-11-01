use tiny_keccak::{Hasher, Keccak};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

fn keccak256_with_nonce(input: &[u8], nonce: u64) -> Vec<u8> {
    let mut hasher = Keccak::v256();
    hasher.update(input);
    hasher.update(&nonce.to_le_bytes()); // Use the nonce as part of the input
    let mut output = vec![0u8; 32];
    hasher.finalize(&mut output);
    output
}

fn has_matching_prefix(hash: &[u8], prefix_zeros: usize) -> bool {
    let required_bytes = prefix_zeros / 8;
    let remaining_bits = prefix_zeros % 8;

    // Check if first `required_bytes` are all zero
    if hash[..required_bytes].iter().any(|&byte| byte != 0) {
        return false;
    }
    
    // Check remaining bits if any
    if remaining_bits > 0 {
        let mask = 0xFF << (8 - remaining_bits);
        return hash[required_bytes] & mask == 0;
    }
    true
}

fn find_nonce_with_prefix(input: &[u8], prefix: &[u8]) -> Option<(u64, Vec<u8>)> {
    let found = Arc::new(AtomicBool::new(false));
    let result_nonce = Arc::new(AtomicU64::new(0));
    let result_hash = Arc::new(parking_lot::RwLock::new(Vec::new()));
    
    let num_threads = num_cpus::get();
    let threads: Vec<_> = (0..num_threads).map(|thread_id| {
        let input = input.to_vec();
        let prefix = prefix.to_vec();
        let found = Arc::clone(&found);
        let result_nonce = Arc::clone(&result_nonce);
        let result_hash = Arc::clone(&result_hash);
        
        thread::spawn(move || {
            let mut hasher = Keccak::v256();
            let mut hash = [0u8; 32];
            let mut nonce = thread_id as u64;
            
            while !found.load(Ordering::Relaxed) {
                hasher.update(&input);
                hasher.update(&nonce.to_le_bytes());
                hasher.finalize(&mut hash);
                
                if hash.starts_with(&prefix) {
                    if !found.swap(true, Ordering::Relaxed) {
                        result_nonce.store(nonce, Ordering::Relaxed);
                        *result_hash.write() = hash.to_vec();
                    }
                    break;
                }
                nonce += num_threads as u64; // Skip by number of threads
                hasher = Keccak::v256();
            }
        })
    }).collect();

    for thread in threads {
        thread.join().unwrap();
    }

    if found.load(Ordering::Relaxed) {
        Some((result_nonce.load(Ordering::Relaxed), result_hash.read().clone()))
    } else {
        None
    }
}

fn main() {
    let input = b"Hello, Keccak!";
    let prefix = b"\x00\x00\x00\x00";
    
    let start = Instant::now();
    match find_nonce_with_prefix(input, prefix) {
        Some((nonce, hash)) => {
            println!("Found nonce: {}", nonce);
            println!("Hash: {:x?}", hash);
        }
        None => println!("No valid nonce found."),
    }
    println!("Time elapsed: {:?}", start.elapsed());
}
