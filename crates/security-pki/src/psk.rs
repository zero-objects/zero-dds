// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Built-in pre-shared-key authentication plugin (spec §10.7).
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (`SharedSecretProvider` is a plugin SPI; the `&dyn` test bridge demonstrates
//! the substitution path to the crypto plugin.)
//!
//! Spec class ID `"DDS:Auth:PSK:1.2"`. Alternative to the X.509 PKI path
//! from `crate::plugin` — the identity is established via a pre-shared
//! symmetric key (typically 256 bits) instead of via a
//! cert chain. Use case: embedded systems without an X.509
//! toolchain (industrial / defense / mesh networks).
//!
//! # Wire layout (spec §10.7.2)
//!
//! Three tokens, all as `DataHolder` on the wire:
//!
//! | Token  | class_id                       | properties / binary properties |
//! |--------|--------------------------------|--------------------------------|
//! | Req    | `DDS:Auth:PSK:1.2+AuthReq`     | `psk.id`, `challenge1`, `c.kagree_algo="PSK"` |
//! | Reply  | `DDS:Auth:PSK:1.2+AuthReply`   | `psk.id`, `challenge1`, `challenge2`, `hmac` |
//! | Final  | `DDS:Auth:PSK:1.2+AuthFinal`   | `challenge1`, `challenge2`, `hmac` |
//!
//! The HMAC is `HMAC-SHA256(pre_shared_key, length_prefixed(psk.id ||
//! challenge1 || challenge2))`. The `SharedSecret` output is 32 bytes
//! HKDF-SHA256(pre_shared_key, salt=challenge1||challenge2,
//! info="DDS-Security-1.2-PSK").
//!
//! # Spec items not covered here
//!
//! * Token-specific `properties` from spec §10.7 Tab.61 — the spec
//!   lists a few optional properties (e.g. `c.id` as identity,
//!   we map that to `psk.id`). We follow the Cyclone-DDS
//!   convention path instead of the spec's lettering.
//! * Cross-vendor live interop is untested — the Cyclone PSK suite
//!   is not openly available.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::backend::hkdf;
use crate::backend::hmac;
use crate::backend::rand::{SecureRandom, SystemRandom};
use zerodds_security::authentication::{
    AuthenticationPlugin, HandshakeHandle, HandshakeStepOutcome, IdentityHandle,
    SharedSecretHandle, SharedSecretProvider,
};
use zerodds_security::error::{SecurityError, SecurityErrorKind, SecurityResult};
use zerodds_security::properties::PropertyList;
use zerodds_security::token::DataHolder;

/// Spec-conformant class-ID strings (§10.7).
pub mod class_id {
    /// Plugin class ID of the PSK authentication.
    pub const PSK: &str = "DDS:Auth:PSK:1.2";
    /// `HandshakeRequestMessageToken` PSK.
    pub const REQUEST: &str = "DDS:Auth:PSK:1.2+AuthReq";
    /// `HandshakeReplyMessageToken` PSK.
    pub const REPLY: &str = "DDS:Auth:PSK:1.2+AuthReply";
    /// `HandshakeFinalMessageToken` PSK.
    pub const FINAL: &str = "DDS:Auth:PSK:1.2+AuthFinal";
}

/// Property keys in the handshake token (spec §10.7.2).
pub mod prop {
    /// PSK identity (UTF-8 string).
    pub const PSK_ID: &str = "psk.id";
    /// Initiator challenge (32 bytes binary).
    pub const CHALLENGE1: &str = "challenge1";
    /// Replier challenge (32 bytes binary).
    pub const CHALLENGE2: &str = "challenge2";
    /// HMAC-SHA256 tag (32 bytes binary).
    pub const HMAC: &str = "hmac";
    /// Key agreement algorithm (always `"PSK"` for this plugin).
    pub const KAGREE_ALGO: &str = "c.kagree_algo";
}

/// PropertyList key for the local PSK identifier.
pub const PROP_PSK_ID: &str = "dds.psk.identity_id";
/// PropertyList key for the local PSK material (hex-encoded).
pub const PROP_PSK_KEY_HEX: &str = "dds.psk.pre_shared_key_hex";

/// Replay cache per local identity (DoS cap analogous to PKI).
const REPLAY_CACHE_CAP: usize = 1024;

/// HKDF info string for SharedSecret derivation. Spec domain
/// separator (§10.7.3).
pub const HKDF_INFO_SHARED_SECRET: &[u8] = b"DDS-Security-1.2-PSK";

/// Built-in PSK authentication plugin (spec §10.7).
pub struct PskAuthenticationPlugin {
    next_handle: AtomicU64,
    /// Configured PSKs: identity string → pre-shared-key bytes.
    psks: BTreeMap<String, Vec<u8>>,
    /// Locally validated identities: handle → identity string.
    identities: BTreeMap<IdentityHandle, String>,
    /// Initiator state between request and reply.
    pending_initiator: BTreeMap<HandshakeHandle, InitiatorState>,
    /// Replier state between reply and final.
    pending_replier: BTreeMap<HandshakeHandle, ReplierState>,
    /// Completed handshakes → SharedSecret handle.
    handshake_to_secret: BTreeMap<HandshakeHandle, SharedSecretHandle>,
    /// Materialized SharedSecrets (32 bytes HKDF output).
    secrets: BTreeMap<SharedSecretHandle, Vec<u8>>,
    /// Replay cache: per local identity the already-seen
    /// `challenge1` values (replier view).
    replay_cache: BTreeMap<IdentityHandle, BTreeSet<[u8; 32]>>,
    /// FIFO order of the replay-cache entries for cap eviction.
    replay_order: BTreeMap<IdentityHandle, Vec<[u8; 32]>>,
}

struct InitiatorState {
    local: IdentityHandle,
    psk_id: String,
    challenge1: [u8; 32],
}

struct ReplierState {
    local: IdentityHandle,
    psk_id: String,
    challenge1: [u8; 32],
    challenge2: [u8; 32],
    secret_handle: SharedSecretHandle,
}

impl Default for PskAuthenticationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PskAuthenticationPlugin {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(0),
            psks: BTreeMap::new(),
            identities: BTreeMap::new(),
            pending_initiator: BTreeMap::new(),
            pending_replier: BTreeMap::new(),
            handshake_to_secret: BTreeMap::new(),
            secrets: BTreeMap::new(),
            replay_cache: BTreeMap::new(),
            replay_order: BTreeMap::new(),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Registers a pre-shared key for an identity. Replace
    /// semantics: a second registration with the same ID overwrites.
    ///
    /// # Errors
    /// `BadArgument` if `id` is empty or `key` is empty.
    pub fn register_psk(&mut self, id: String, key: Vec<u8>) -> SecurityResult<()> {
        if id.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: identity-id empty",
            ));
        }
        if key.is_empty() {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                "psk: pre-shared-key empty",
            ));
        }
        self.psks.insert(id, key);
        Ok(())
    }

    /// Validates a local PSK identity and returns a handle.
    ///
    /// # Errors
    /// `BadArgument` if `identity_id` is not registered.
    pub fn validate_local_psk_identity(
        &mut self,
        identity_id: &str,
    ) -> SecurityResult<IdentityHandle> {
        if !self.psks.contains_key(identity_id) {
            return Err(SecurityError::new(
                SecurityErrorKind::BadArgument,
                alloc::format!("psk: unknown identity-id '{identity_id}'"),
            ));
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, identity_id.to_string());
        Ok(handle)
    }

    /// Validates a remote PSK identity (from the propagated
    /// IdentityToken). Check: the claimed `psk.id` must be present in
    /// our local PSK map.
    ///
    /// # Errors
    /// `BadArgument` if the token has a format error or the
    /// `psk.id` is not in the local map.
    pub fn validate_remote_psk_identity(
        &mut self,
        remote_token: &[u8],
    ) -> SecurityResult<IdentityHandle> {
        let dh = DataHolder::from_cdr_le(remote_token)?;
        if dh.class_id != class_id::PSK {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!(
                    "psk: remote IdentityToken has wrong class_id '{}'",
                    dh.class_id
                ),
            ));
        }
        let id = dh.property(PROP_PSK_ID).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: IdentityToken without psk.id",
            )
        })?;
        if !self.psks.contains_key(id) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: remote psk.id '{id}' not in the local trust store"),
            ));
        }
        let handle = IdentityHandle(self.next_id());
        self.identities.insert(handle, id.to_string());
        Ok(handle)
    }

    /// Creates the wire IdentityToken (`DDS:Auth:PSK:1.2`) for a
    /// local identity.
    ///
    /// # Errors
    /// `BadArgument` if the handle is unknown.
    pub fn build_identity_token(&self, local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        let id = self.identities.get(&local).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "psk: unknown handle")
        })?;
        let dh = DataHolder::new(class_id::PSK).with_property(PROP_PSK_ID, id.clone());
        Ok(dh.to_cdr_le())
    }

    /// Returns the raw SharedSecret (32 bytes) — primarily for tests.
    #[must_use]
    pub fn secret_bytes(&self, handle: SharedSecretHandle) -> Option<&[u8]> {
        self.secrets.get(&handle).map(Vec::as_slice)
    }

    fn store_secret(&mut self, bytes: Vec<u8>) -> SharedSecretHandle {
        let handle = SharedSecretHandle(self.next_id());
        self.secrets.insert(handle, bytes);
        handle
    }

    fn record_challenge(&mut self, local: IdentityHandle, c: [u8; 32]) -> SecurityResult<()> {
        let cache = self.replay_cache.entry(local).or_default();
        if cache.contains(&c) {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: replayed challenge1 detected",
            ));
        }
        cache.insert(c);
        let order = self.replay_order.entry(local).or_default();
        order.push(c);
        if order.len() > REPLAY_CACHE_CAP {
            let dropped = order.remove(0);
            cache.remove(&dropped);
        }
        Ok(())
    }

    fn lookup_psk(&self, id: &str) -> SecurityResult<&[u8]> {
        self.psks.get(id).map(Vec::as_slice).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: unknown identity-id '{id}'"),
            )
        })
    }
}

impl SharedSecretProvider for PskAuthenticationPlugin {
    fn get_shared_secret(&self, handle: SharedSecretHandle) -> Option<Vec<u8>> {
        self.secrets.get(&handle).cloned()
    }
}

/// Random 32-byte challenge.
fn random_challenge() -> SecurityResult<[u8; 32]> {
    let rng = SystemRandom::new();
    let mut buf = [0u8; 32];
    rng.fill(&mut buf).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::CryptoFailed,
            "psk: SystemRandom not available",
        )
    })?;
    Ok(buf)
}

/// HMAC input per spec §10.7.2. Length-prefixed concatenation
/// (`u32-LE len || bytes`) prevents cross-field tampering.
fn hmac_input(psk_id: &str, ch1: &[u8; 32], ch2: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + psk_id.len() + 4 + 32 + 4 + 32);
    out.extend_from_slice(&(psk_id.len() as u32).to_le_bytes());
    out.extend_from_slice(psk_id.as_bytes());
    out.extend_from_slice(&(32u32).to_le_bytes());
    out.extend_from_slice(ch1);
    out.extend_from_slice(&(32u32).to_le_bytes());
    out.extend_from_slice(ch2);
    out
}

fn hmac_sign(psk: &[u8], psk_id: &str, ch1: &[u8; 32], ch2: &[u8; 32]) -> [u8; 32] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, psk);
    let tag = hmac::sign(&key, &hmac_input(psk_id, ch1, ch2));
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

fn hmac_verify(
    psk: &[u8],
    psk_id: &str,
    ch1: &[u8; 32],
    ch2: &[u8; 32],
    tag: &[u8],
) -> SecurityResult<()> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, psk);
    hmac::verify(&key, &hmac_input(psk_id, ch1, ch2), tag).map_err(|_| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            "psk: hmac verify failed",
        )
    })
}

/// Spec §10.7.3 — SharedSecret = HKDF-SHA256(psk, salt=ch1||ch2,
/// info="DDS-Security-1.2-PSK"). Output: 32 bytes.
pub fn derive_psk_shared_secret(
    psk: &[u8],
    ch1: &[u8; 32],
    ch2: &[u8; 32],
) -> SecurityResult<[u8; 32]> {
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(ch1);
    salt[32..].copy_from_slice(ch2);
    let salt_obj = hkdf::Salt::new(hkdf::HKDF_SHA256, &salt);
    let prk = salt_obj.extract(psk);
    let info = [HKDF_INFO_SHARED_SECRET];
    let okm = prk.expand(&info, hkdf::HKDF_SHA256).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "psk: HKDF expand failed")
    })?;
    let mut out = [0u8; 32];
    okm.fill(&mut out).map_err(|_| {
        SecurityError::new(SecurityErrorKind::CryptoFailed, "psk: HKDF fill failed")
    })?;
    Ok(out)
}

fn read_32(dh: &DataHolder, name: &str) -> SecurityResult<[u8; 32]> {
    let bytes = dh.binary_property(name).ok_or_else(|| {
        SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("psk: missing binary property '{name}'"),
        )
    })?;
    if bytes.len() != 32 {
        return Err(SecurityError::new(
            SecurityErrorKind::AuthenticationFailed,
            alloc::format!("psk: '{name}' must be 32 bytes"),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Ok(out)
}

impl AuthenticationPlugin for PskAuthenticationPlugin {
    fn validate_local_identity(
        &mut self,
        props: &PropertyList,
        _participant_guid: [u8; 16],
    ) -> SecurityResult<IdentityHandle> {
        let id = props.get(PROP_PSK_ID).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::InvalidConfiguration,
                "psk: missing dds.psk.identity_id",
            )
        })?;
        // Optional: load PSK material directly from properties (hex).
        if let Some(hex) = props.get(PROP_PSK_KEY_HEX) {
            let bytes = hex_decode(hex)?;
            self.register_psk(id.to_string(), bytes)?;
        }
        self.validate_local_psk_identity(id)
    }

    /// Returns the `IdentityToken` announced in the SPDP beacon (PID_IDENTITY_TOKEN).
    /// Without this trait impl the default `Ok(Vec::new())` would remain → no token on the
    /// beacon → the peer never starts `begin_handshake_with` (no-op on token=None)
    /// → the PSK handshake never happens live. Delegates to `build_identity_token`.
    fn get_identity_token(&self, local: IdentityHandle) -> SecurityResult<Vec<u8>> {
        self.build_identity_token(local)
    }

    fn validate_remote_identity(
        &mut self,
        _local: IdentityHandle,
        _remote_participant_guid: [u8; 16],
        remote_auth_token: &[u8],
    ) -> SecurityResult<IdentityHandle> {
        self.validate_remote_psk_identity(remote_auth_token)
    }

    fn begin_handshake_request(
        &mut self,
        initiator: IdentityHandle,
        _replier: IdentityHandle,
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let psk_id = self
            .identities
            .get(&initiator)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk: unknown initiator IdentityHandle",
                )
            })?
            .clone();
        let challenge1 = random_challenge()?;

        let token = DataHolder::new(class_id::REQUEST)
            .with_property(prop::PSK_ID, psk_id.clone())
            .with_property(prop::KAGREE_ALGO, "PSK")
            .with_binary_property(prop::CHALLENGE1, challenge1.to_vec())
            .to_cdr_le();

        let handle = HandshakeHandle(self.next_id());
        self.pending_initiator.insert(
            handle,
            InitiatorState {
                local: initiator,
                psk_id,
                challenge1,
            },
        );
        Ok((handle, HandshakeStepOutcome::SendMessage { token }))
    }

    fn begin_handshake_reply(
        &mut self,
        replier: IdentityHandle,
        _initiator: IdentityHandle,
        request_token: &[u8],
    ) -> SecurityResult<(HandshakeHandle, HandshakeStepOutcome)> {
        let dh = DataHolder::from_cdr_le(request_token)?;
        if dh.class_id != class_id::REQUEST {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!(
                    "psk: reply expected request, got class_id '{}'",
                    dh.class_id
                ),
            ));
        }
        let psk_id = dh
            .property(prop::PSK_ID)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "psk: request missing psk.id",
                )
            })?
            .to_string();
        let challenge1 = read_32(&dh, prop::CHALLENGE1)?;

        // Replay detection (replier side).
        self.record_challenge(replier, challenge1)?;

        // PSK lookup against the local trust store.
        let psk = self.lookup_psk(&psk_id)?.to_vec();

        let challenge2 = random_challenge()?;
        let hmac = hmac_sign(&psk, &psk_id, &challenge1, &challenge2);
        let secret = derive_psk_shared_secret(&psk, &challenge1, &challenge2)?;

        let token = DataHolder::new(class_id::REPLY)
            .with_property(prop::PSK_ID, psk_id.clone())
            .with_property(prop::KAGREE_ALGO, "PSK")
            .with_binary_property(prop::CHALLENGE1, challenge1.to_vec())
            .with_binary_property(prop::CHALLENGE2, challenge2.to_vec())
            .with_binary_property(prop::HMAC, hmac.to_vec())
            .to_cdr_le();

        let secret_handle = self.store_secret(secret.to_vec());
        let handle = HandshakeHandle(self.next_id());
        self.handshake_to_secret.insert(handle, secret_handle);
        self.pending_replier.insert(
            handle,
            ReplierState {
                local: replier,
                psk_id,
                challenge1,
                challenge2,
                secret_handle,
            },
        );
        Ok((handle, HandshakeStepOutcome::SendMessage { token }))
    }

    fn process_handshake(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        if self.pending_initiator.contains_key(&handshake) {
            return self.process_reply_on_initiator(handshake, token);
        }
        if self.pending_replier.contains_key(&handshake) {
            return self.process_final_on_replier(handshake, token);
        }
        Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk: unknown HandshakeHandle",
        ))
    }

    fn shared_secret(&self, handshake: HandshakeHandle) -> SecurityResult<SharedSecretHandle> {
        self.handshake_to_secret
            .get(&handshake)
            .copied()
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::BadArgument,
                    "psk: handshake handle unknown or not yet completed",
                )
            })
    }

    fn plugin_class_id(&self) -> &str {
        class_id::PSK
    }
}

impl PskAuthenticationPlugin {
    fn process_reply_on_initiator(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let dh = DataHolder::from_cdr_le(token)?;
        if dh.class_id != class_id::REPLY {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: process expected reply, got '{}'", dh.class_id),
            ));
        }
        let st = self.pending_initiator.remove(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "psk: initiator state gone")
        })?;
        let psk_id_in = dh
            .property(prop::PSK_ID)
            .ok_or_else(|| {
                SecurityError::new(
                    SecurityErrorKind::AuthenticationFailed,
                    "psk: reply missing psk.id",
                )
            })?
            .to_string();
        if psk_id_in != st.psk_id {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: psk.id echo mismatch in reply",
            ));
        }
        let ch1 = read_32(&dh, prop::CHALLENGE1)?;
        if ch1 != st.challenge1 {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: challenge1 echo mismatch",
            ));
        }
        let ch2 = read_32(&dh, prop::CHALLENGE2)?;
        let hmac = dh.binary_property(prop::HMAC).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: reply missing hmac",
            )
        })?;

        let psk = self.lookup_psk(&st.psk_id)?.to_vec();
        hmac_verify(&psk, &st.psk_id, &ch1, &ch2, hmac)?;

        // Own HMAC for the final token.
        let final_hmac = hmac_sign(&psk, &st.psk_id, &ch1, &ch2);
        let secret = derive_psk_shared_secret(&psk, &ch1, &ch2)?;
        let secret_handle = self.store_secret(secret.to_vec());
        self.handshake_to_secret.insert(handshake, secret_handle);

        let final_token = DataHolder::new(class_id::FINAL)
            .with_binary_property(prop::CHALLENGE1, ch1.to_vec())
            .with_binary_property(prop::CHALLENGE2, ch2.to_vec())
            .with_binary_property(prop::HMAC, final_hmac.to_vec())
            .to_cdr_le();

        let _ = st.local;
        Ok(HandshakeStepOutcome::SendMessage { token: final_token })
    }

    fn process_final_on_replier(
        &mut self,
        handshake: HandshakeHandle,
        token: &[u8],
    ) -> SecurityResult<HandshakeStepOutcome> {
        let dh = DataHolder::from_cdr_le(token)?;
        if dh.class_id != class_id::FINAL {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                alloc::format!("psk: process expected final, got '{}'", dh.class_id),
            ));
        }
        let st = self.pending_replier.remove(&handshake).ok_or_else(|| {
            SecurityError::new(SecurityErrorKind::BadArgument, "psk: replier state gone")
        })?;
        let ch1 = read_32(&dh, prop::CHALLENGE1)?;
        let ch2 = read_32(&dh, prop::CHALLENGE2)?;
        if ch1 != st.challenge1 || ch2 != st.challenge2 {
            return Err(SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: final challenge echo mismatch",
            ));
        }
        let hmac = dh.binary_property(prop::HMAC).ok_or_else(|| {
            SecurityError::new(
                SecurityErrorKind::AuthenticationFailed,
                "psk: final missing hmac",
            )
        })?;
        let psk = self.lookup_psk(&st.psk_id)?.to_vec();
        hmac_verify(&psk, &st.psk_id, &ch1, &ch2, hmac)?;

        let _ = st.local;
        Ok(HandshakeStepOutcome::Complete {
            secret: st.secret_handle,
        })
    }
}

fn hex_decode(s: &str) -> SecurityResult<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk: hex string with odd length",
        ));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for chunk in bytes.chunks(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        // Arithmetic form instead of `(hi << 4) | lo`: for nibble values
        // (0..=15) the bits do not overlap, so it is identical.
        // mutation-detection-friendly: `*` and `+` are not
        // equivalent to each other.
        out.push(hi * 16 + lo);
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> SecurityResult<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(SecurityError::new(
            SecurityErrorKind::BadArgument,
            "psk: invalid hex nibble",
        )),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security::properties::Property;

    fn alice_bob_with_shared_psk() -> (
        PskAuthenticationPlugin,
        PskAuthenticationPlugin,
        IdentityHandle,
        IdentityHandle,
    ) {
        let psk = alloc::vec![0xA5u8; 32];
        let mut alice = PskAuthenticationPlugin::new();
        let mut bob = PskAuthenticationPlugin::new();
        alice.register_psk("alice-bob".into(), psk.clone()).unwrap();
        bob.register_psk("alice-bob".into(), psk).unwrap();
        let alice_h = alice.validate_local_psk_identity("alice-bob").unwrap();
        let bob_h = bob.validate_local_psk_identity("alice-bob").unwrap();
        (alice, bob, alice_h, bob_h)
    }

    #[test]
    fn plugin_class_id_matches_spec() {
        let p = PskAuthenticationPlugin::new();
        assert_eq!(p.plugin_class_id(), "DDS:Auth:PSK:1.2");
    }

    #[test]
    fn token_class_ids_match_spec() {
        assert_eq!(class_id::PSK, "DDS:Auth:PSK:1.2");
        assert_eq!(class_id::REQUEST, "DDS:Auth:PSK:1.2+AuthReq");
        assert_eq!(class_id::REPLY, "DDS:Auth:PSK:1.2+AuthReply");
        assert_eq!(class_id::FINAL, "DDS:Auth:PSK:1.2+AuthFinal");
    }

    #[test]
    fn register_psk_then_validate_local_happy_path() {
        let mut p = PskAuthenticationPlugin::new();
        p.register_psk("client-1".into(), alloc::vec![0x11; 32])
            .unwrap();
        let h = p.validate_local_psk_identity("client-1").unwrap();
        assert!(h.0 >= 1);
    }

    #[test]
    fn validate_local_unknown_id_rejected() {
        let mut p = PskAuthenticationPlugin::new();
        let err = p.validate_local_psk_identity("ghost").unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_rejects_empty_key() {
        let mut p = PskAuthenticationPlugin::new();
        let err = p.register_psk("x".into(), Vec::new()).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_rejects_empty_id() {
        let mut p = PskAuthenticationPlugin::new();
        let err = p
            .register_psk(String::new(), alloc::vec![1, 2, 3])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn register_psk_replace_semantics_last_wins() {
        let mut p = PskAuthenticationPlugin::new();
        p.register_psk("k".into(), alloc::vec![1; 32]).unwrap();
        p.register_psk("k".into(), alloc::vec![2; 32]).unwrap();
        let key = p.psks.get("k").unwrap();
        assert_eq!(key, &alloc::vec![2u8; 32]);
    }

    #[test]
    fn full_three_round_handshake_alice_bob() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();

        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        let (bob_hs, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!("expected SendMessage"),
        };

        let out4 = bob.process_handshake(bob_hs, &final_tok).unwrap();
        let bob_secret = match out4 {
            HandshakeStepOutcome::Complete { secret } => secret,
            _ => panic!("expected Complete"),
        };

        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        let a_bytes = alice.secret_bytes(alice_secret).unwrap();
        let b_bytes = bob.secret_bytes(bob_secret).unwrap();
        assert_eq!(a_bytes.len(), 32);
        assert_eq!(a_bytes, b_bytes, "alice + bob must have the same secret");
    }

    #[test]
    fn tampered_reply_hmac_rejected_by_initiator() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let mut reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let mut h = DataHolder::from_cdr_le(&reply).unwrap();
        let mut hmac = h.binary_property(prop::HMAC).unwrap().to_vec();
        hmac[0] ^= 0x01;
        h.set_binary_property(prop::HMAC, hmac);
        reply = h.to_cdr_le();
        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn replay_initiator_request_rejected_second_time() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (_alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let err = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn wrong_psk_on_replier_breaks_hmac_on_initiator() {
        let mut alice = PskAuthenticationPlugin::new();
        let mut bob = PskAuthenticationPlugin::new();
        alice
            .register_psk("k".into(), alloc::vec![0xAAu8; 32])
            .unwrap();
        // Bob has a different key for the same ID.
        bob.register_psk("k".into(), alloc::vec![0xBBu8; 32])
            .unwrap();
        let alice_h = alice.validate_local_psk_identity("k").unwrap();
        let bob_h = bob.validate_local_psk_identity("k").unwrap();

        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (_, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let err = alice.process_handshake(alice_hs, &reply).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn unknown_psk_id_in_request_rejected_by_replier() {
        let mut bob = PskAuthenticationPlugin::new();
        bob.register_psk("known".into(), alloc::vec![0x11; 32])
            .unwrap();
        let bob_h = bob.validate_local_psk_identity("known").unwrap();
        // Alice sends a request with an "unknown" id (hand-built).
        let req = DataHolder::new(class_id::REQUEST)
            .with_property(prop::PSK_ID, "unknown")
            .with_property(prop::KAGREE_ALGO, "PSK")
            .with_binary_property(prop::CHALLENGE1, alloc::vec![0u8; 32])
            .to_cdr_le();
        let err = bob
            .begin_handshake_reply(bob_h, IdentityHandle(99), &req)
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn truncated_request_rejected() {
        let mut bob = PskAuthenticationPlugin::new();
        bob.register_psk("k".into(), alloc::vec![0x11; 32]).unwrap();
        let bob_h = bob.validate_local_psk_identity("k").unwrap();
        let err = bob
            .begin_handshake_reply(bob_h, IdentityHandle(99), &[0u8, 1, 2])
            .unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn validate_remote_token_happy_path() {
        let mut p = PskAuthenticationPlugin::new();
        p.register_psk("peer-1".into(), alloc::vec![0xCCu8; 32])
            .unwrap();
        let local = p.validate_local_psk_identity("peer-1").unwrap();
        let token = p.build_identity_token(local).unwrap();
        let remote = p.validate_remote_psk_identity(&token).unwrap();
        assert_ne!(remote, local);
    }

    #[test]
    fn validate_remote_token_rejects_unknown_id() {
        let mut p = PskAuthenticationPlugin::new();
        p.register_psk("known".into(), alloc::vec![0xCCu8; 32])
            .unwrap();
        let token = DataHolder::new(class_id::PSK)
            .with_property(PROP_PSK_ID, "stranger")
            .to_cdr_le();
        let err = p.validate_remote_psk_identity(&token).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn validate_remote_token_rejects_wrong_class_id() {
        let mut p = PskAuthenticationPlugin::new();
        p.register_psk("k".into(), alloc::vec![0x1u8; 32]).unwrap();
        let token = DataHolder::new("DDS:Auth:PKI-DH:1.2")
            .with_property(PROP_PSK_ID, "k")
            .to_cdr_le();
        let err = p.validate_remote_psk_identity(&token).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn cross_plugin_psk_vs_pki_mismatch_class_id() {
        // Token from the PSK plugin → classified as different by the
        // PKI IdentityToken decoder (different class_ids).
        let mut psk = PskAuthenticationPlugin::new();
        psk.register_psk("k".into(), alloc::vec![0xAA; 32]).unwrap();
        let h = psk.validate_local_psk_identity("k").unwrap();
        let psk_token = psk.build_identity_token(h).unwrap();
        let dh = DataHolder::from_cdr_le(&psk_token).unwrap();
        assert_eq!(dh.class_id, "DDS:Auth:PSK:1.2");
        assert_ne!(dh.class_id, "DDS:Auth:PKI-DH:1.2");
    }

    #[test]
    fn token_roundtrip_via_data_holder_codec() {
        let mut p = PskAuthenticationPlugin::new();
        p.register_psk("alpha".into(), alloc::vec![0xBE; 32])
            .unwrap();
        let h = p.validate_local_psk_identity("alpha").unwrap();
        let token = p.build_identity_token(h).unwrap();
        let dh = DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(dh.class_id, class_id::PSK);
        assert_eq!(dh.property(PROP_PSK_ID), Some("alpha"));
    }

    #[test]
    fn validate_local_via_property_list_with_inline_hex_key() {
        let mut p = PskAuthenticationPlugin::new();
        let key_hex: String = (0..32).map(|_| "ab").collect();
        let props = PropertyList::new()
            .with(Property::local(PROP_PSK_ID, "node-1"))
            .with(Property::local(PROP_PSK_KEY_HEX, key_hex));
        let h = p.validate_local_identity(&props, [0xAA; 16]).unwrap();
        assert!(h.0 >= 1);
    }

    #[test]
    fn validate_local_via_property_list_missing_id_rejected() {
        let mut p = PskAuthenticationPlugin::new();
        let props = PropertyList::new();
        let err = p.validate_local_identity(&props, [0xAA; 16]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::InvalidConfiguration);
    }

    #[test]
    fn shared_secret_returns_bad_argument_for_unknown_handle() {
        let p = PskAuthenticationPlugin::new();
        let err = p.shared_secret(HandshakeHandle(42)).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn hkdf_test_vector_rfc5869_ish_is_deterministic() {
        // Cross-validation: same PSK + same challenges → bit-identical secret.
        let psk = alloc::vec![0x0bu8; 22];
        let ch1 = [0x01u8; 32];
        let ch2 = [0x02u8; 32];
        let s1 = derive_psk_shared_secret(&psk, &ch1, &ch2).unwrap();
        let s2 = derive_psk_shared_secret(&psk, &ch1, &ch2).unwrap();
        assert_eq!(s1, s2);
        // Different challenges → different secret.
        let s3 = derive_psk_shared_secret(&psk, &ch2, &ch1).unwrap();
        assert_ne!(s1, s3);
    }

    #[test]
    fn shared_secret_is_32_bytes() {
        let psk = alloc::vec![0xFFu8; 16];
        let s = derive_psk_shared_secret(&psk, &[0u8; 32], &[1u8; 32]).unwrap();
        assert_eq!(s.len(), 32);
    }

    #[test]
    fn process_handshake_unknown_handle_rejected() {
        let mut p = PskAuthenticationPlugin::new();
        let err = p.process_handshake(HandshakeHandle(999), &[]).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn final_token_validates_initiator_hmac_on_replier() {
        // If the initiator HMAC in the final token is broken,
        // process_final_on_replier fails with AuthenticationFailed.
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, req_out) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match req_out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, reply_out) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let reply = match reply_out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let final_out = alice.process_handshake(alice_hs, &reply).unwrap();
        let mut final_tok = match final_out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let mut h = DataHolder::from_cdr_le(&final_tok).unwrap();
        let mut hm = h.binary_property(prop::HMAC).unwrap().to_vec();
        hm[5] ^= 0xFF;
        h.set_binary_property(prop::HMAC, hm);
        final_tok = h.to_cdr_le();
        let err = bob.process_handshake(bob_hs, &final_tok).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn request_token_carries_kagree_psk() {
        let (mut alice, _bob, alice_h, _bob_h) = alice_bob_with_shared_psk();
        let (_, out) = alice
            .begin_handshake_request(alice_h, IdentityHandle(99))
            .unwrap();
        let token = match out {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let dh = DataHolder::from_cdr_le(&token).unwrap();
        assert_eq!(dh.class_id, class_id::REQUEST);
        assert_eq!(dh.property(prop::KAGREE_ALGO), Some("PSK"));
    }

    #[test]
    fn shared_secret_provider_returns_bytes_after_handshake() {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        bob.process_handshake(bob_hs, &final_tok).unwrap();

        let alice_secret = alice.shared_secret(alice_hs).unwrap();
        let provider: &dyn SharedSecretProvider = &alice;
        let bytes = provider.get_shared_secret(alice_secret).unwrap();
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn hex_decode_round_trips_simple_input() {
        let v = hex_decode("0a0b").unwrap();
        assert_eq!(v, alloc::vec![0x0a, 0x0b]);
    }

    #[test]
    fn hex_decode_rejects_odd_len() {
        let err = hex_decode("abc").unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    #[test]
    fn hex_decode_rejects_non_hex() {
        let err = hex_decode("zz").unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::BadArgument);
    }

    // -------------------------------------------------------------
    // Mutation killers (2026-05-01)
    // -------------------------------------------------------------

    /// Replay-cache CAP boundary (analogous to plugin.rs).
    /// Catches `>` -> `==`/`>=` on record_challenge eviction.
    #[test]
    fn psk_replay_cache_holds_exactly_cap() {
        let (mut alice, _, alice_h, _) = alice_bob_with_shared_psk();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        let mut c_first = [0u8; 32];
        c_first[0..8].copy_from_slice(&0u64.to_le_bytes());
        let err = alice.record_challenge(alice_h, c_first).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn psk_replay_cache_evicts_at_cap_plus_one() {
        let (mut alice, _, alice_h, _) = alice_bob_with_shared_psk();
        for i in 0..REPLAY_CACHE_CAP {
            let mut c = [0u8; 32];
            c[0..8].copy_from_slice(&(i as u64).to_le_bytes());
            alice.record_challenge(alice_h, c).unwrap();
        }
        let mut c_extra = [0u8; 32];
        c_extra[0..8].copy_from_slice(&(REPLAY_CACHE_CAP as u64).to_le_bytes());
        alice.record_challenge(alice_h, c_extra).unwrap();
        let mut c0 = [0u8; 32];
        c0[0..8].copy_from_slice(&0u64.to_le_bytes());
        alice
            .record_challenge(alice_h, c0)
            .expect("oldest should be evicted");
    }

    /// hmac_input returns a specific length-prefixed concatenation.
    /// Catches mutations `vec![]`/`vec![0]`/`vec![1]`.
    #[test]
    fn hmac_input_length_prefixed_concatenation() {
        let psk_id = "abc";
        let ch1 = [0x11u8; 32];
        let ch2 = [0x22u8; 32];
        let out = hmac_input(psk_id, &ch1, &ch2);
        // Layout: u32-LE psk_id_len + psk_id + u32-LE 32 + ch1 + u32-LE 32 + ch2
        // = 4 + 3 + 4 + 32 + 4 + 32 = 79
        assert_eq!(out.len(), 4 + 3 + 4 + 32 + 4 + 32);
        // First 4 bytes: 3 as u32-LE
        assert_eq!(&out[0..4], &3u32.to_le_bytes());
        // psk_id ascii
        assert_eq!(&out[4..7], b"abc");
        // ch1 len prefix
        assert_eq!(&out[7..11], &32u32.to_le_bytes());
        // ch1 bytes
        assert_eq!(&out[11..43], &ch1[..]);
        // ch2 len prefix
        assert_eq!(&out[43..47], &32u32.to_le_bytes());
        // ch2 bytes
        assert_eq!(&out[47..79], &ch2[..]);

        // Different inputs => different outputs
        let out_b = hmac_input("xyz", &ch1, &ch2);
        assert_ne!(out, out_b);
    }

    /// hex_nibble: cover all three branches a..f, A..F, 0..9.
    /// Catches `delete arm` and `+ -> -` mutations.
    #[test]
    fn hex_nibble_all_three_ranges() {
        for (c, expected) in [
            (b'0', 0u8),
            (b'5', 5),
            (b'9', 9),
            (b'a', 10),
            (b'c', 12),
            (b'f', 15),
            (b'A', 10),
            (b'C', 12),
            (b'F', 15),
        ] {
            assert_eq!(hex_nibble(c).unwrap(), expected, "char {c:#x}");
        }
        assert!(hex_nibble(b'g').is_err());
        assert!(hex_nibble(b'G').is_err());
        assert!(hex_nibble(b'@').is_err());
    }

    /// hex_decode with concrete value — catches `*` -> `+` and arith.
    /// mutations on the nibble accumulation.
    #[test]
    fn hex_decode_specific_byte_values() {
        // "ab" = 0xAB; with `*` mutated the result would be 10+11=21 != 171.
        assert_eq!(hex_decode("ab").unwrap(), vec![0xAB]);
        // "F0" = 0xF0; mutated: 15+0=15 vs 15*16=240.
        assert_eq!(hex_decode("F0").unwrap(), vec![0xF0]);
        assert_eq!(hex_decode("0F").unwrap(), vec![0x0F]);
        assert_eq!(
            hex_decode("DEADBEEF").unwrap(),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    /// Final-token tampering: a single field mismatch must reject.
    /// Catches `||` -> `&&` (line 624).
    fn run_psk_final_tampered<M>(mutate: M)
    where
        M: FnOnce(&mut DataHolder),
    {
        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let mut h = DataHolder::from_cdr_le(&final_tok).unwrap();
        mutate(&mut h);
        let tampered = h.to_cdr_le();
        let err = bob.process_handshake(bob_hs, &tampered).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }

    #[test]
    fn psk_final_challenge1_tamper_rejected() {
        run_psk_final_tampered(|h| {
            let mut v = h.binary_property("challenge1").unwrap().to_vec();
            v[0] ^= 0x01;
            h.set_binary_property("challenge1", v);
        });
    }
    #[test]
    fn psk_final_challenge2_tamper_rejected() {
        run_psk_final_tampered(|h| {
            let mut v = h.binary_property("challenge2").unwrap().to_vec();
            v[0] ^= 0x01;
            h.set_binary_property("challenge2", v);
        });
    }

    /// Catches `||` -> `&&` (line 624) in process_final_on_replier
    /// echo check: ch1 != local AND ch2 == local must reject.
    ///
    /// Naive tamper tests do not catch this, because the HMAC verify ALSO
    /// catches the mismatch. Here the HMAC is recomputed over the tampered
    /// values, so that the mutation to `&&` leads to replier
    /// acceptance while the original `||` leads to an echo reject.
    #[test]
    fn psk_final_only_ch1_tamper_with_recomputed_hmac_rejected() {
        let psk_bytes = alloc::vec![0xA5u8; 32];
        let psk_id = "alice-bob";

        let (mut alice, mut bob, alice_h, bob_h) = alice_bob_with_shared_psk();
        let (alice_hs, out1) = alice.begin_handshake_request(alice_h, bob_h).unwrap();
        let req = match out1 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let (bob_hs, out2) = bob.begin_handshake_reply(bob_h, alice_h, &req).unwrap();
        let reply = match out2 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };
        let out3 = alice.process_handshake(alice_hs, &reply).unwrap();
        let final_tok = match out3 {
            HandshakeStepOutcome::SendMessage { token } => token,
            _ => panic!(),
        };

        // Tamper challenge1 + recompute HMAC over the tampered values.
        let mut h = DataHolder::from_cdr_le(&final_tok).unwrap();
        let mut new_ch1 = h.binary_property("challenge1").unwrap().to_vec();
        new_ch1[0] ^= 0x01;
        let ch2_vec = h.binary_property("challenge2").unwrap().to_vec();

        let mut ch1_arr = [0u8; 32];
        ch1_arr.copy_from_slice(&new_ch1);
        let mut ch2_arr = [0u8; 32];
        ch2_arr.copy_from_slice(&ch2_vec);
        let new_hmac = hmac_sign(&psk_bytes, psk_id, &ch1_arr, &ch2_arr);

        h.set_binary_property("challenge1", new_ch1);
        h.set_binary_property("hmac", new_hmac.to_vec());
        let tampered = h.to_cdr_le();

        // Original (`||`): echo mismatch of ch1 => AuthFailed.
        // Mutation (`&&`): ch1 mismatched but ch2 ok => the echo check
        // passes, the HMAC verify passes (recomputed) => success.
        let err = bob.process_handshake(bob_hs, &tampered).unwrap_err();
        assert_eq!(err.kind, SecurityErrorKind::AuthenticationFailed);
    }
}
