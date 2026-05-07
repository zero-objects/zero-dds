//! Stable-Rust Fuzz-Smoke-Tests fuer XTypes-Wire-Decoder.
//!
//! Spec: DDS-XTypes 1.3 §7.3 (TypeIdentifier), §7.4 (TypeObject),
//! §7.5 (TypeInformation), §7.7 (TypeLookup-Service).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_types::{TypeIdentifier, TypeInformation, TypeObject};

#[derive(Debug, Clone)]
struct XorShift32(u32);
impl XorShift32 {
    fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF } else { seed })
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}
fn random_bytes(rng: &mut XorShift32, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        out.extend_from_slice(&rng.next_u32().to_le_bytes());
    }
    out.truncate(len);
    out
}

fn fuzz_decoder<F: FnMut(&[u8])>(seed: u32, iterations: usize, mut f: F) {
    let mut rng = XorShift32::new(seed);
    for i in 0..iterations {
        let len = match i % 8 {
            0 => 0,
            1 => 1,
            2 => 8,
            3 => 16,
            4 => 64,
            5 => 256,
            6 => 1024,
            _ => 4096,
        };
        let bytes = random_bytes(&mut rng, len);
        f(&bytes);
    }
}

#[test]
fn fuzz_type_identifier_from_bytes_no_panic() {
    fuzz_decoder(0x5449_4444, 3_000, |bytes| {
        let _ = TypeIdentifier::from_bytes_le(bytes);
    });
}

#[test]
fn fuzz_type_information_from_bytes_no_panic() {
    fuzz_decoder(0x5449_494E, 3_000, |bytes| {
        let _ = TypeInformation::from_bytes_le(bytes);
    });
}

#[test]
fn fuzz_type_object_from_bytes_no_panic() {
    fuzz_decoder(0x544F_424A, 3_000, |bytes| {
        let _ = TypeObject::from_bytes_le(bytes);
    });
}

#[test]
fn empty_inputs_no_panic() {
    let _ = TypeIdentifier::from_bytes_le(&[]);
    let _ = TypeInformation::from_bytes_le(&[]);
    let _ = TypeObject::from_bytes_le(&[]);
}

#[test]
fn single_byte_inputs_no_panic() {
    for b in 0u8..=255 {
        let _ = TypeIdentifier::from_bytes_le(&[b]);
        let _ = TypeInformation::from_bytes_le(&[b]);
        let _ = TypeObject::from_bytes_le(&[b]);
    }
}
