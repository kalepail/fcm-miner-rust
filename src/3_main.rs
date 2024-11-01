use hex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tiny_keccak::{Hasher, Keccak};

const DIFFICULTY: usize = 9;
const BATCH_SIZE: u64 = 10_000_000;

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

#[inline(always)]
fn calculate_hash(data: &[u8]) -> [u8; 32] {
    let mut hash = [0u8; 32];
    let mut hasher = Keccak::v256();
    hasher.update(data);
    hasher.finalize(&mut hash);
    hash
}

fn mine_block(block_data: BlockData) -> (u64, [u8; 32]) {
    let found = Arc::new(AtomicBool::new(false));
    let hash_count = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();
    let mut handles = vec![];
    let num_threads = num_cpus::get();

    for thread_id in 0..num_threads {
        let mut block_data = block_data.clone();
        let found = found.clone();
        let hash_count = hash_count.clone();

        handles.push(thread::spawn(move || {
            let mut hash = [0u8; 32];
            let mut local_hash_count = 0u64;
            let mut nonce = thread_id as u64;

            while !found.load(Ordering::Relaxed) {
                // Update nonce in-place
                block_data.combined_data[block_data.nonce_position..][..8]
                    .copy_from_slice(&nonce.to_be_bytes());

                let mut hasher = Keccak::v256();
                hasher.update(&block_data.combined_data);
                hasher.finalize(&mut hash);

                if check_difficulty(&hash) {
                    found.store(true, Ordering::Release);
                    return Some(nonce);
                }

                nonce += num_threads as u64;
                local_hash_count += 1;

                if local_hash_count >= BATCH_SIZE {
                    hash_count.fetch_add(local_hash_count, Ordering::Relaxed);
                    local_hash_count = 0;
                }
            }
            None
        }));
    }

    let solution = handles
        .into_iter()
        .find_map(|h| h.join().unwrap())
        .expect("Solution should be found");

    // Calculate final hash with winning nonce
    let mut final_data = block_data.clone();
    final_data.combined_data[final_data.nonce_position..][..8]
        .copy_from_slice(&solution.to_be_bytes());
    let final_hash = calculate_hash(&final_data.combined_data);

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
        1438,
        "KALE",
        &[
            0,0,0,0,65,251,25,114,70,203,227,146,34,46,222,31,210,70,180,73,66,224,61,126,67,84,223,10,65,221,197,211
        ],
        &[
            71, 91, 242, 164, 88, 135, 40, 119, 138, 130, 113, 54, 158, 224, 57, 86, 17, 3, 255,
            206, 53, 73, 64, 44, 224, 164, 121, 206, 191, 27, 9, 245,
        ],
    );

    let (nonce, hash) = mine_block(block_data);
    println!("Solution nonce: {}", nonce);
    println!("Hash: 0x{}", hex::encode(hash));
}
