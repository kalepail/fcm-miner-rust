use std::thread;
use tiny_keccak::{Hasher, Keccak};
use clap::Parser;

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

fn build_prefix(index: u64, prev_hash: [u8; 32]) -> Vec<u8> {
    let message = "KALE";

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

fn main() {
    let args = Args::parse();

    let index = args.index;
    let prev_hash = hex::decode(args.prev_hash).unwrap().try_into().unwrap();
    let target_zeros = args.target_zeros;

    let num_threads = num_cpus::get();

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let prefix = build_prefix(index, prev_hash);
        let suffix = build_suffix();
        let handle = thread::spawn(move || {
            let mut hash = [0u8; 32];
            let mut nonce = thread_id;
            let mut hasher_buffer = vec![0u8; prefix.len() + 8 + suffix.len()];

            // Copy static data into buffer
            hasher_buffer[..prefix.len()].copy_from_slice(&prefix);
            hasher_buffer[prefix.len() + 8..].copy_from_slice(&suffix);

            loop {
                for _ in 0..BATCH_SIZE {
                    hasher_buffer[prefix.len()..prefix.len() + 8].copy_from_slice(&nonce.to_be_bytes());

                    let mut keccak = Keccak::v256();
                    keccak.update(&hasher_buffer);
                    keccak.finalize(&mut hash);

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
