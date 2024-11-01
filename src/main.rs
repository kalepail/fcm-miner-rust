use clap::Parser;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use tiny_keccak::{Hasher, Keccak};
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 50_000;
const BATCH_SIZE_U64: u64 = BATCH_SIZE as u64;

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
        71, 91, 242, 164, 88, 135, 40, 119, 138, 130, 113, 54, 158, 224, 57, 86, 17, 3, 255, 206,
        53, 73, 64, 44, 224, 164, 121, 206, 191, 27, 9, 245,
    ];

    let mut suffix = [0; 44];

    suffix[..12].copy_from_slice(&[0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]);
    suffix[12..].copy_from_slice(&miner);

    suffix
}

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

fn main() {
    let counter = Arc::new(AtomicU64::new(0));
    // let start_time = Instant::now();

    let args = Args::parse();

    let index = args.index;
    let prev_hash = hex::decode(args.prev_hash).unwrap().try_into().unwrap();
    let target_zeros = args.target_zeros;

    let num_threads = num_cpus::get();

    let mut handles = vec![];

    let prefix = build_prefix(index, prev_hash);
    let suffix = build_suffix();
    let mut hasher_buffer = [0; 68 + 8 + 44];

    // Copy static data into buffer
    hasher_buffer[..prefix.len()].copy_from_slice(&prefix);
    hasher_buffer[prefix.len() + 8..].copy_from_slice(&suffix);

    // Spawn worker threads
    for thread_id in 0..num_threads {
        let counter = counter.clone();

        let handle = thread::spawn(move || {
            let mut hash = [0u8; 32];
            let mut nonce_start = thread_id;

            loop {
                let nonce_end = nonce_start + BATCH_SIZE;

                // Process entire batch
                for nonce in nonce_start..nonce_end {
                    hasher_buffer[68..68 + 8].copy_from_slice(&nonce.to_be_bytes());

                    let mut keccak = Keccak::v256();
                    keccak.update(&hasher_buffer);
                    keccak.finalize(&mut hash);

                    if count_leading_hex_zeros(&hash) == target_zeros {
                        println!("[{}, \"{}\"]", nonce, hex::encode(hash));
                        counter.fetch_add(1, Ordering::Relaxed); // Count the final hash
                        std::process::exit(0);
                    }
                }

                // Update counter with batch size after processing
                counter.fetch_add(BATCH_SIZE_U64, Ordering::Relaxed);

                // Update start nonce
                nonce_start = nonce_end;
            }
        });

        handles.push(handle);
    }

    // Monitor hashrate
    let report_thread = thread::spawn({
        let counter = counter.clone();

        move || {
            let mut last_counter = 0u64;
            let mut last_time = Instant::now();

            loop {
                thread::sleep(Duration::from_secs(2));

                let current = counter.load(Ordering::Relaxed);
                let elapsed = last_time.elapsed().as_secs_f64();
                let hashes = (current - last_counter) as f64;

                // Convert to MH/s
                let hashrate = hashes / elapsed / 1_000_000.0;

                println!("Hashrate: {:.2} MH/s", hashrate);

                last_counter = current;
                last_time = Instant::now();
            }
        }
    });

    // Wait for solution
    for handle in handles {
        handle.join().unwrap();
    }

    report_thread.join().unwrap();

    // let elapsed = start_time.elapsed();
    // let total_hashes = counter.load(Ordering::Relaxed);

    // println!("Found solution in {:?}", elapsed);
    // println!("Total hashes: {}", total_hashes);
    // println!("Average hashrate: {:.2} MH/s",
    //     (total_hashes as f64 / elapsed.as_secs_f64()) / 1_000_000.0);
}
