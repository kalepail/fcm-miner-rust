use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tiny_keccak::{Hasher, Keccak};
use clap::Parser;

// Add at top level
static HASH_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Block index
    #[arg(short, long)]
    index: u64,

    /// Previous block hash (hex string)
    #[arg(short, long)]
    prev_hash: String,

    /// Number of leading zeros required
    #[arg(short, long)]
    target_zeros: usize,
}

const BATCH_SIZE: usize = 50_000;

fn count_leading_hex_zeros(hash: &[u8]) -> usize {
    // First 8 bytes
    let first_u64 = u64::from_be_bytes([
        hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
    ]);

    let leading_zeros = first_u64.leading_zeros();
    let first_count = (leading_zeros as usize) / 4;

    // If we used all first 8 bytes, check next 8
    if first_count == 16 {
        let second_u64 = u64::from_be_bytes([
            hash[8], hash[9], hash[10], hash[11], hash[12], hash[13], hash[14], hash[15],
        ]);
        first_count + (second_u64.leading_zeros() as usize) / 4
    } else {
        first_count
    }
}

fn build_prefix(index: u64, prev_hash: [u8; 32]) -> [u8; 68] {
    let message = "KALE";

    let mut prefix = [0; 68];

    prefix[..4].copy_from_slice(&[0, 0, 0, 5]);
    prefix[4..12].copy_from_slice(&index.to_be_bytes());

    prefix[12..20].copy_from_slice(&[0, 0, 0, 14, 0, 0, 0, 4]);
    prefix[20..24].copy_from_slice(message.as_bytes());

    prefix[24..32].copy_from_slice(&[0, 0, 0, 13, 0, 0, 0, 32]);
    prefix[32..64].copy_from_slice(&prev_hash);

    prefix[64..].copy_from_slice(&[0, 0, 0, 5]);

    prefix
}

fn build_suffix() -> [u8; 44] {
    let miner: [u8; 32] = [
        71, 91, 242, 164, 88, 135, 40, 119, 138, 130, 113, 54, 158, 224, 57, 86,
        17, 3, 255, 206, 53, 73, 64, 44, 224, 164, 121, 206, 191, 27, 9, 245,
    ];

    let mut suffix = [0; 44];

    suffix[..12].copy_from_slice(&[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
    suffix[12..].copy_from_slice(&miner);
    
    suffix
}

// Add this function
fn start_hashrate_logger() {
    thread::spawn(|| {
        let mut last_time = Instant::now();
        let mut last_count = 0u64;

        loop {
            thread::sleep(Duration::from_secs(2));
            let current_count = HASH_COUNT.load(Ordering::Relaxed);
            let current_time = Instant::now();
            
            let elapsed = current_time.duration_since(last_time).as_secs_f64();
            let hashes = current_count - last_count;
            let hashrate = hashes as f64 / elapsed;

            println!("Hashrate: {:.2} MH/s", hashrate / 1_000_000.0);
            
            last_count = current_count;
            last_time = current_time;
        }
    });
}

fn main() {
    let args = Args::parse();

    let index = args.index;
    let prev_hash = hex::decode(args.prev_hash).unwrap().try_into().unwrap();
    let target_zeros = args.target_zeros;

    let num_threads = num_cpus::get();

    let mut handles = vec![];

    // Add in main() before the main loop:
    start_hashrate_logger();

    let prefix = build_prefix(index, prev_hash);
    let suffix = build_suffix();
    let mut hasher_buffer = [0; 68 + 8 + 44];

    // Copy static data into buffer
    hasher_buffer[..prefix.len()].copy_from_slice(&prefix);
    hasher_buffer[prefix.len() + 8..].copy_from_slice(&suffix);

    for thread_id in 0..num_threads {
        let handle = thread::spawn(move || {
            let mut hash = [0u8; 32];
            let mut nonce = thread_id;

            loop {
                for _ in 0..BATCH_SIZE {
                    // let mut hb = hasher_buffer.clone();
                    // hb[68..68 + 8].copy_from_slice(&nonce.to_be_bytes());

                    let mut keccak = Keccak::v256();
                    // keccak.update(&hb);
                    keccak.update(&nonce.to_be_bytes());
                    keccak.finalize(&mut hash);

                    // Add in your hashing loop:
                    HASH_COUNT.fetch_add(1, Ordering::Relaxed);

                    if count_leading_hex_zeros(&hash) >= target_zeros {
                        println!(
                            "[{}, \"{}\"]",
                            nonce,
                            hex::encode(hash)
                        );
                        std::process::exit(0);
                    }

                    nonce += num_threads;
                }
            }
        });

        handles.push(handle);
    }

    // Wait for threads (though we'll exit before this if solution found)
    for handle in handles {
        let _ = handle.join();
    }
}
