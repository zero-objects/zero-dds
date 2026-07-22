//! Per-protection-kind crypto overhead — the SEC_BODY payload transform cost
//! that `data_protection_kind` buys per sample: Encrypt (AEAD) vs Sign
//! (integrity only) vs the no-crypto baseline, across payload sizes. The active
//! crypto backend is the compiled feature (ring / aws-lc / wolfcrypt).
//!
//! This is the measured half of the O9 per-topic-security-overhead item: run it
//! on the Linux bench host and read off ns/op + throughput per kind.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_security::authentication::IdentityHandle;
use zerodds_security::crypto::{CryptoHandle, CryptographicPlugin};
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};

const SIZES: &[usize] = &[64, 256, 1024, 4096, 16384];

fn setup(suite: Suite) -> (AesGcmCryptoPlugin, CryptoHandle) {
    let mut p = AesGcmCryptoPlugin::with_suite(suite);
    let h = p
        .register_local_participant(IdentityHandle(1), &[])
        .expect("register local participant");
    (p, h)
}

fn bench_payload_protection(c: &mut Criterion) {
    let mut g = c.benchmark_group("payload_protection_encode");
    for &size in SIZES {
        let payload = vec![0xABu8; size];
        g.throughput(Throughput::Bytes(size as u64));

        // NONE: the per-sample cost with no protection is just handing the
        // payload on — an owned copy is the honest floor to compare against.
        g.bench_with_input(BenchmarkId::new("none", size), &payload, |b, pl| {
            b.iter(|| black_box(pl.clone()));
        });

        // ENCRYPT (AES-128-GCM) and ENCRYPT (AES-256-GCM): integrity +
        // confidentiality. The SEC_BODY payload transform is AEAD; the
        // integrity-only (SIGN) kinds go through the submessage-MAC path, not
        // `encode_serialized_payload`, so they are not measured here.
        for (label, suite) in [
            ("encrypt_aes128_gcm", Suite::Aes128Gcm),
            ("encrypt_aes256_gcm", Suite::Aes256Gcm),
        ] {
            let (p, h) = setup(suite);
            g.bench_with_input(BenchmarkId::new(label, size), &payload, |b, pl| {
                b.iter(|| black_box(p.encode_serialized_payload(h, black_box(pl)).unwrap()));
            });
        }
    }
    g.finish();
}

criterion_group!(benches, bench_payload_protection);
criterion_main!(benches);
