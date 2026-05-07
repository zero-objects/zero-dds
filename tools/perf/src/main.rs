// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Load generator, latency profiler, benchmark suite.
//!
//! Binary `zerodds-perf`.
//!
//! Phase-5 D.2 Sub-Commands:
//!
//! * `zerodds-perf hw-info` — druckt HW-Crypto-Capabilities (CPU-Features
//!   + Backend-Label).
//! * `zerodds-perf aes-gcm [--bytes N] [--block N] [--suite 128|256]` —
//!   AES-GCM Throughput-Bench. Default: 100 MiB Total, 1 KiB Block,
//!   AES-128-GCM.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::time::Instant;

use ring::aead::{AES_128_GCM, AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use zerodds_security_crypto::aes_gcm_hw;

fn main() {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());

    let rc = match cmd.as_str() {
        "hw-info" => cmd_hw_info(),
        "aes-gcm" => cmd_aes_gcm(args.collect()),
        "monitor" => cmd_monitor(),
        "help" | "--help" | "-h" => {
            print_help();
            0
        }
        other => {
            eprintln!("zerodds-perf: unknown command `{other}`");
            print_help();
            2
        }
    };
    std::process::exit(rc);
}

fn cmd_monitor() -> i32 {
    // Triggert ein paar Counter/Histogramm-Updates ueber den
    // Crypto-Hot-Path und druckt anschliessend die Prometheus-
    // Exposition (zerodds-monitor-1.0 §3).
    use zerodds_monitor::{Labels, default_registry, metric_names};

    let reg = default_registry();
    reg.set_help(
        metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
        "Synthetisch durch zerodds-perf monitor",
    );
    reg.counter(
        metric_names::DDS_DCPS_SAMPLES_WRITTEN_TOTAL,
        Labels::new().with("topic", "perf.synthetic"),
    )
    .add(42);
    reg.histogram(
        metric_names::DDS_DCPS_SAMPLE_LATENCY_SECONDS,
        Labels::new().with("topic", "perf.synthetic"),
    )
    .record_ns(150_000);

    print!("{}", reg.render_prometheus());
    0
}

fn print_help() {
    println!("zerodds-perf — ZeroDDS Performance Tooling");
    println!();
    println!("USAGE:");
    println!("    zerodds-perf <COMMAND> [ARGS]");
    println!();
    println!("COMMANDS:");
    println!("    hw-info                              CPU-Features + AES-GCM-Backend");
    println!("    aes-gcm [--bytes N] [--block N]      AES-GCM Throughput-Bench");
    println!("            [--suite 128|256]");
    println!("    monitor                              zerodds-monitor::Registry-Snapshot");
    println!(
        "                                         als Prometheus-Text (zerodds-monitor-1.0 §3)"
    );
    println!("    help                                 Diese Hilfe");
}

fn cmd_hw_info() -> i32 {
    let r = aes_gcm_hw::report();
    println!("{r}");
    0
}

#[derive(Debug, Clone, Copy)]
enum AesSuite {
    Aes128,
    Aes256,
}

impl AesSuite {
    fn key_len(self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes256 => 32,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Aes128 => "AES-128-GCM",
            Self::Aes256 => "AES-256-GCM",
        }
    }
    fn algorithm(self) -> &'static ring::aead::Algorithm {
        match self {
            Self::Aes128 => &AES_128_GCM,
            Self::Aes256 => &AES_256_GCM,
        }
    }
}

fn cmd_aes_gcm(argv: Vec<String>) -> i32 {
    let mut total_bytes: u64 = 100 * 1024 * 1024; // 100 MiB
    let mut block_size: usize = 1024; // 1 KiB
    let mut suite = AesSuite::Aes128;

    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--bytes" => match it.next().and_then(|s| s.parse::<u64>().ok()) {
                Some(n) if n > 0 => total_bytes = n,
                _ => {
                    eprintln!("zerodds-perf aes-gcm: --bytes requires a positive integer");
                    return 2;
                }
            },
            "--block" => match it.next().and_then(|s| s.parse::<usize>().ok()) {
                Some(n) if (16..=1 << 20).contains(&n) => block_size = n,
                _ => {
                    eprintln!("zerodds-perf aes-gcm: --block must be 16..=1048576");
                    return 2;
                }
            },
            "--suite" => match it.next().as_deref() {
                Some("128") => suite = AesSuite::Aes128,
                Some("256") => suite = AesSuite::Aes256,
                _ => {
                    eprintln!("zerodds-perf aes-gcm: --suite must be `128` or `256`");
                    return 2;
                }
            },
            other => {
                eprintln!("zerodds-perf aes-gcm: unknown arg `{other}`");
                return 2;
            }
        }
    }

    run_aes_gcm_bench(total_bytes, block_size, suite)
}

fn run_aes_gcm_bench(total_bytes: u64, block_size: usize, suite: AesSuite) -> i32 {
    let caps = aes_gcm_hw::cached();
    println!(
        "zerodds-perf aes-gcm: suite={} block={}B total={}B backend={}",
        suite.label(),
        block_size,
        total_bytes,
        caps.backend_label()
    );

    // Master-Key + Buffer pre-allokieren (out of timing).
    let mut key_bytes = vec![0u8; suite.key_len()];
    for (i, b) in key_bytes.iter_mut().enumerate() {
        *b = (i & 0xFF) as u8;
    }
    let unbound = match UnboundKey::new(suite.algorithm(), &key_bytes) {
        Ok(k) => k,
        Err(_) => {
            eprintln!("zerodds-perf aes-gcm: UnboundKey rejected");
            return 1;
        }
    };
    let key = LessSafeKey::new(unbound);

    let mut block: Vec<u8> = vec![0u8; block_size];
    for (i, b) in block.iter_mut().enumerate() {
        *b = (i & 0xFF) as u8;
    }
    let mut buf = Vec::with_capacity(block_size + 16);

    let iters = total_bytes / (block_size as u64);
    if iters == 0 {
        eprintln!("zerodds-perf aes-gcm: total < block, nothing to bench");
        return 2;
    }

    // ENCRYPT-Pfad.
    let mut nonce_counter: u64 = 0;
    let aad = b"zerodds-perf";
    let start = Instant::now();
    for _ in 0..iters {
        nonce_counter = nonce_counter.wrapping_add(1);
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..].copy_from_slice(&nonce_counter.to_be_bytes());
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        buf.clear();
        buf.extend_from_slice(&block);
        if key
            .seal_in_place_append_tag(nonce, Aad::from(aad), &mut buf)
            .is_err()
        {
            eprintln!("zerodds-perf aes-gcm: seal failed at iter {nonce_counter}");
            return 1;
        }
    }
    let elapsed_enc = start.elapsed();
    let mb_per_s_enc = (total_bytes as f64) / (1024.0 * 1024.0) / elapsed_enc.as_secs_f64();
    println!("  encrypt: {mb_per_s_enc:>8.2} MiB/s   ({iters} blocks, {elapsed_enc:?})");

    // DECRYPT-Pfad.
    let last_ct = buf.clone();
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes[4..].copy_from_slice(&nonce_counter.to_be_bytes());
    let start = Instant::now();
    for _ in 0..iters {
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut decbuf = last_ct.clone();
        if key
            .open_in_place(nonce, Aad::from(aad), &mut decbuf)
            .is_err()
        {
            eprintln!("zerodds-perf aes-gcm: open failed");
            return 1;
        }
    }
    let elapsed_dec = start.elapsed();
    let mb_per_s_dec = (total_bytes as f64) / (1024.0 * 1024.0) / elapsed_dec.as_secs_f64();
    println!("  decrypt: {mb_per_s_dec:>8.2} MiB/s   ({iters} blocks, {elapsed_dec:?})");

    0
}
