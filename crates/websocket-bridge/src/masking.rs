// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket payload masking — RFC 6455 §5.3.

/// Spec §5.3 (p. 32): "octet i of the transformed data ('transformed-
/// octet-i') is the XOR of octet i of the original data ('original-
/// octet-i') with octet at index i modulo 4 of the masking key
/// ('masking-key-octet-j')".
///
/// This operation is symmetric — the same call masks **and**
/// unmasks.
pub fn apply_mask(data: &mut [u8], key: [u8; 4]) {
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= key[i & 0x3];
    }
}

/// The random source could not deliver a masking key. A client that cannot
/// draw a key cannot send the frame: RFC 6455 §5.3 allows no frame from a
/// client without a key from a strong source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyError;

impl core::fmt::Display for EntropyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("the random source delivered no masking key")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EntropyError {}

/// A masking key from the operating system's random source. Spec §5.3
/// (p. 32): "The masking key is a 32-bit value chosen at random by the
/// client. [...] The masking key needs to be unpredictable; thus, the
/// masking key MUST be derived from a strong source of entropy, and the
/// masking key for a given frame MUST NOT make it simple for a server/proxy
/// to predict the masking key for a subsequent frame."
///
/// Each call reads four octets from the operating system (`getrandom`); no
/// state of this crate takes part, so one key says nothing about the next.
///
/// # Errors
/// [`EntropyError`] when the operating system delivers no random octets.
#[cfg(feature = "std")]
pub fn generate_masking_key() -> Result<[u8; 4], EntropyError> {
    let mut key = [0u8; 4];
    getrandom::getrandom(&mut key).map_err(|_| EntropyError)?;
    Ok(key)
}

/// The source of a client's masking keys (Spec §5.3, §10.3): one call, one
/// key for one frame.
///
/// With `std` the crate's source is [`OsRngProvider`]. A `no_std` caller
/// implements the trait over the random source of its platform (a hardware
/// generator, a CSPRNG seeded from one); the crate ships no generator of
/// its own, because one that is not seeded from a strong source is exactly
/// what §5.3 excludes.
pub trait MaskingKeyProvider {
    /// A new 32-bit masking key.
    ///
    /// # Errors
    /// [`EntropyError`] when the source delivers none.
    fn next_key(&mut self) -> Result<[u8; 4], EntropyError>;
}

/// The operating system's random source ([`generate_masking_key`]).
#[cfg(feature = "std")]
#[derive(Debug, Default, Clone, Copy)]
pub struct OsRngProvider;

#[cfg(feature = "std")]
impl MaskingKeyProvider for OsRngProvider {
    fn next_key(&mut self) -> Result<[u8; 4], EntropyError> {
        generate_masking_key()
    }
}

/// Caller-supplied provider — wraps an `FnMut` that returns a key.
/// Lets applications inject their own sources (a hardware RNG, a CSPRNG)
/// without writing their own trait impl.
pub struct ClosureMaskingKeyProvider<F: FnMut() -> Result<[u8; 4], EntropyError>>(pub F);

impl<F: FnMut() -> Result<[u8; 4], EntropyError>> MaskingKeyProvider
    for ClosureMaskingKeyProvider<F>
{
    fn next_key(&mut self) -> Result<[u8; 4], EntropyError> {
        (self.0)()
    }
}

/// Which end of the connection a frame was received on — the masking
/// rule of RFC 6455 §5.1 is directional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The receiver is the client; its peer is the server.
    Client,
    /// The receiver is the server; its peer is the client.
    Server,
}

/// Violation of the §5.1 masking rule on a received frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskingViolation {
    /// Spec §5.1: "The server MUST close the connection upon
    /// receiving a frame that is not masked."
    UnmaskedFrameFromClient,
    /// Spec §5.1: "A client MUST close a connection if it detects a
    /// masked frame."
    MaskedFrameFromServer,
}

impl core::fmt::Display for MaskingViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnmaskedFrameFromClient => write!(f, "UnmaskedFrameFromClient"),
            Self::MaskedFrameFromServer => write!(f, "MaskedFrameFromServer"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MaskingViolation {}

impl MaskingViolation {
    /// Spec §7.4.1 — the status code the connection is failed with.
    /// A masking violation is a protocol error (1002).
    #[must_use]
    pub const fn close_code(self) -> crate::close::CloseCode {
        crate::close::CloseCode::ProtocolError
    }
}

/// Spec §5.1, p. 27 — "The client MUST mask all frames that it sends
/// to the server. […] A server MUST NOT mask any frames that it
/// sends to the client." Checks a *received* frame against that rule
/// from the point of view of `receiver`.
///
/// # Errors
/// [`MaskingViolation`] when the frame violates the directional rule.
pub const fn validate_masking(
    frame: &crate::frame::Frame,
    receiver: Role,
) -> Result<(), MaskingViolation> {
    check_masking(frame.masking_key.is_some(), receiver)
}

/// [`validate_masking`] on the MASK bit of a frame header alone, before
/// the frame is complete.
///
/// # Errors
/// [`MaskingViolation`] when the bit violates the directional rule.
pub const fn check_masking(masked: bool, receiver: Role) -> Result<(), MaskingViolation> {
    match (receiver, masked) {
        (Role::Server, false) => Err(MaskingViolation::UnmaskedFrameFromClient),
        (Role::Client, true) => Err(MaskingViolation::MaskedFrameFromServer),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_5_1_client_masks_and_server_does_not_per_spec_5_1() {
        use crate::frame::Frame;
        // Spec §5.1: a frame from the client is masked, a frame from
        // the server is not, and each end rejects the other case.
        let from_client = Frame::text("hi").with_mask([1, 2, 3, 4]);
        let from_server = Frame::text("hi");
        assert_eq!(validate_masking(&from_client, Role::Server), Ok(()));
        assert_eq!(validate_masking(&from_server, Role::Client), Ok(()));
        assert_eq!(
            validate_masking(&from_server, Role::Server),
            Err(MaskingViolation::UnmaskedFrameFromClient)
        );
        assert_eq!(
            validate_masking(&from_client, Role::Client),
            Err(MaskingViolation::MaskedFrameFromServer)
        );
    }

    #[test]
    fn ws_5_1_masking_violation_fails_the_connection_with_1002() {
        // Spec §5.1 demands the connection be closed; §7.4.1 gives
        // 1002 "Protocol error" as its status code.
        assert_eq!(
            MaskingViolation::UnmaskedFrameFromClient
                .close_code()
                .to_u16(),
            1002
        );
        assert_eq!(
            MaskingViolation::MaskedFrameFromServer
                .close_code()
                .to_u16(),
            1002
        );
    }

    #[test]
    fn ws_5_3_apply_mask_is_symmetric() {
        // Spec §5.3 — XOR is self-inverse.
        let key = [0x01, 0x02, 0x03, 0x04];
        let original: alloc::vec::Vec<u8> = (0..16).collect();
        let mut masked = original.clone();
        apply_mask(&mut masked, key);
        assert_ne!(masked, original);
        apply_mask(&mut masked, key);
        assert_eq!(masked, original);
    }

    #[test]
    fn ws_5_3_apply_mask_xors_with_key_modulo_4() {
        // Spec §5.3 — index modulo 4.
        let key = [0xFF, 0x00, 0xFF, 0x00];
        let mut data = [0xAA; 8];
        apply_mask(&mut data, key);
        assert_eq!(data, [0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA]);
    }

    #[test]
    fn ws_5_3_apply_mask_handles_partial_key_alignment() {
        // Spec §5.3 — a payload length that is not a multiple of 4 must
        // work correctly.
        let key = [0x01, 0x02, 0x03, 0x04];
        let mut data = [0x10, 0x20, 0x30, 0x40, 0x50];
        apply_mask(&mut data, key);
        // Index 0..3 = key[0..3], Index 4 = key[0].
        assert_eq!(data[0], 0x10 ^ 0x01);
        assert_eq!(data[1], 0x20 ^ 0x02);
        assert_eq!(data[2], 0x30 ^ 0x03);
        assert_eq!(data[3], 0x40 ^ 0x04);
        assert_eq!(data[4], 0x50 ^ 0x01);
    }

    #[test]
    fn ws_5_3_empty_payload_is_unchanged() {
        let mut data: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        apply_mask(&mut data, [1, 2, 3, 4]);
        assert!(data.is_empty());
    }

    #[cfg(feature = "std")]
    /// The generator this crate had before: Splitmix64 over a counter that
    /// started at a constant, so every process drew the same key sequence.
    fn former_fixed_sequence(n: usize) -> alloc::vec::Vec<[u8; 4]> {
        let mut counter: u64 = 0xCAFE_F00D_DEAD_BEEF;
        (0..n)
            .map(|_| {
                let mut x = counter;
                counter = counter.wrapping_add(0x9E37_79B9_7F4A_7C15);
                x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                x ^= x >> 31;
                let b = x.to_le_bytes();
                [b[0], b[1], b[2], b[3]]
            })
            .collect()
    }

    #[cfg(feature = "std")]
    #[test]
    fn ws_5_3_the_masking_key_comes_from_the_operating_system() {
        // §5.3: "MUST be derived from a strong source of entropy". The keys
        // are not the sequence a constant seed gives (what the crate drew
        // before, in every process the same) ...
        let drawn: alloc::vec::Vec<[u8; 4]> = (0..4096)
            .map(|_| generate_masking_key().expect("os random source"))
            .collect();
        let fixed = former_fixed_sequence(4096);
        let same = drawn.iter().zip(&fixed).filter(|(a, b)| a == b).count();
        assert!(same < 4, "{same} of 4096 keys are the fixed sequence");
        // ... every bit of the key is used: over 4096 keys each of the 32
        // bits is set in 40 to 60 percent of them (a fair bit leaves that
        // band with a probability below 1e-36) ...
        for bit in 0..32 {
            let set = drawn
                .iter()
                .filter(|k| (u32::from_le_bytes(**k) >> bit) & 1 == 1)
                .count();
            assert!((1638..=2458).contains(&set), "bit {bit}: {set} of 4096");
        }
        // ... and 4096 keys of 32 bits repeat rarely: the expected number
        // of equal pairs is 0.002, three or more do not happen by chance.
        let mut sorted = drawn.clone();
        sorted.sort_unstable();
        let repeats = sorted.windows(2).filter(|w| w[0] == w[1]).count();
        assert!(repeats < 3, "{repeats} repeated keys in 4096");
    }

    #[cfg(feature = "std")]
    #[test]
    fn ws_5_3_the_os_provider_draws_a_key_per_call() {
        let mut p = OsRngProvider;
        let a = p.next_key().expect("key");
        let b = p.next_key().expect("key");
        let c = p.next_key().expect("key");
        // Three equal 32-bit draws: probability 2^-64.
        assert!(!(a == b && b == c));
    }

    #[test]
    fn ws_5_3_a_source_without_entropy_is_an_error_not_a_key() {
        // A provider that has no key says so; the caller sends no frame.
        let mut p = ClosureMaskingKeyProvider(|| Err(EntropyError));
        assert_eq!(p.next_key(), Err(EntropyError));
    }

    #[test]
    fn ws_5_3_closure_provider_calls_user_supplied_fn() {
        let mut p = ClosureMaskingKeyProvider(|| Ok([9, 9, 9, 9]));
        assert_eq!(p.next_key(), Ok([9, 9, 9, 9]));
    }
}
