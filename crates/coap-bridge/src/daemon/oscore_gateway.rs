// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OSCORE (RFC 8613) gateway for `zerodds-coap-bridged` — Spec §7.2.
//!
//! The daemon is the OSCORE *server*: per client `kid` it derives a
//! Sender/Recipient security context by HKDF from the configured Master
//! Secret/Salt (+ optional ID Context), holds a per-`kid` anti-replay window
//! (§3.2.2, default width 32), unprotects incoming requests (§8.2) and
//! protects responses (§8.3). Direct responses reuse the request nonce
//! (no Partial IV, Appendix C.7); Observe notifications carry the daemon's
//! own Partial IV (§4.1.3.5.2, Appendix C.8) from a per-`kid` sequence.
//!
//! Requires the `oscore` feature; without it the type exists as a stub the
//! daemon refuses to construct (fail closed on `oscore.enabled: true`).

use std::string::String;
use std::vec::Vec;

#[cfg(feature = "oscore")]
use std::collections::BTreeMap;
#[cfg(feature = "oscore")]
use std::sync::Mutex;

#[cfg(feature = "oscore")]
use crate::message::CoapMessage;

#[cfg(feature = "oscore")]
use crate::oscore::message::{OSCORE_OPTION, protect_response_message, unprotect_request_message};
#[cfg(feature = "oscore")]
use crate::oscore::wire::{OscoreOption, ReplayWindow};
#[cfg(feature = "oscore")]
use crate::oscore::{AeadAlgorithm, SecurityContext};

use super::config::OscoreSettings;

/// Why an incoming OSCORE request was rejected (maps to the RFC 8613 §8.2
/// error responses in the server).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OscoreReject {
    /// No OSCORE option — the message is not OSCORE-protected.
    NotProtected,
    /// The Partial IV was already seen (anti-replay, §7.4) → 4.01.
    Replay,
    /// Option malformed or AEAD verification failed (§8.2) → 4.00.
    Verify,
}

/// The verified request parameters a response is bound to (RFC 8613 §8.3:
/// the response AAD carries the request's `kid` + Partial IV).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OscoreExchange {
    /// The client's Sender ID from the request's OSCORE option.
    pub kid: Vec<u8>,
    /// The request's Partial IV.
    pub request_piv: Vec<u8>,
}

/// Per-client-`kid` OSCORE server state — see the module docs.
#[cfg(feature = "oscore")]
pub struct OscoreGateway {
    settings: OscoreSettings,
    peers: Mutex<BTreeMap<Vec<u8>, PeerState>>,
}

#[cfg(feature = "oscore")]
struct PeerState {
    ctx: SecurityContext,
    replay: ReplayWindow,
    /// The daemon's own Partial IV sequence towards this peer (notifications).
    notify_seq: u64,
}

#[cfg(feature = "oscore")]
impl OscoreGateway {
    /// Build the gateway from the validated `oscore:` config block.
    #[must_use]
    pub fn new(settings: OscoreSettings) -> Self {
        Self {
            settings,
            peers: Mutex::new(BTreeMap::new()),
        }
    }

    /// Derive (or fetch the cached) server context for a client `kid`:
    /// Sender ID = the daemon's configured Sender ID, Recipient ID = `kid`.
    fn with_peer<R>(&self, kid: &[u8], f: impl FnOnce(&mut PeerState) -> R) -> Result<R, String> {
        let mut peers = self.peers.lock().map_err(|_| "oscore mutex poisoned")?;
        let state = peers.entry(kid.to_vec()).or_insert_with(|| PeerState {
            ctx: SecurityContext::derive(
                &self.settings.master_secret,
                &self.settings.master_salt,
                &self.settings.sender_id,
                kid,
                self.settings.id_context.as_deref(),
                AeadAlgorithm::AesCcm16_64_128,
            ),
            replay: ReplayWindow::with_width(self.settings.replay_window),
            notify_seq: 0,
        });
        Ok(f(state))
    }

    /// Unprotect an incoming request (RFC 8613 §8.2): decode the OSCORE
    /// option, check the anti-replay window (peek), AEAD-verify + decrypt,
    /// then record the Partial IV. Returns the inner request + the exchange
    /// parameters responses are bound to.
    ///
    /// # Errors
    /// [`OscoreReject`] as `Ok(Err(_))`-style outer `Err(String)` only for
    /// poisoned internal state; protocol rejections come back as
    /// `Ok(Err(reject))`.
    #[allow(clippy::type_complexity)]
    pub fn unprotect_request(
        &self,
        outer: &CoapMessage,
    ) -> Result<Result<(CoapMessage, OscoreExchange), OscoreReject>, String> {
        let Some(osc_opt) = outer.options.iter().find(|o| o.number == OSCORE_OPTION) else {
            return Ok(Err(OscoreReject::NotProtected));
        };
        let Ok(osc) = OscoreOption::decode(&osc_opt.value.to_wire_bytes()) else {
            return Ok(Err(OscoreReject::Verify));
        };
        let kid = osc.kid.clone().unwrap_or_default();
        self.with_peer(&kid, |peer| {
            if !peer.replay.is_fresh(&osc.partial_iv) {
                return Err(OscoreReject::Replay);
            }
            match unprotect_request_message(&peer.ctx, outer) {
                Ok(inner) => {
                    // Only an AEAD-verified Partial IV moves the window.
                    peer.replay.record(&osc.partial_iv);
                    Ok((
                        inner,
                        OscoreExchange {
                            kid: kid.clone(),
                            request_piv: osc.partial_iv.clone(),
                        },
                    ))
                }
                Err(_) => Err(OscoreReject::Verify),
            }
        })
    }

    /// Protect a direct response (RFC 8613 §8.3, no Partial IV — the request
    /// nonce is reused, Appendix C.7).
    ///
    /// # Errors
    /// `Err` on AEAD failure or poisoned internal state.
    pub fn protect_response(
        &self,
        exchange: &OscoreExchange,
        inner: &CoapMessage,
    ) -> Result<CoapMessage, String> {
        let sender_id = self.settings.sender_id.clone();
        self.with_peer(&exchange.kid, |peer| {
            protect_response_message(
                &peer.ctx,
                &sender_id,
                &exchange.kid,
                &exchange.request_piv,
                None,
                inner,
            )
            .map_err(|_| "oscore protect".to_string())
        })?
    }

    /// Protect an Observe notification (RFC 8613 §4.1.3.5.2 — the daemon's
    /// own Partial IV from the per-peer sequence, Appendix C.8).
    ///
    /// # Errors
    /// `Err` on AEAD failure or poisoned internal state.
    pub fn protect_notification(
        &self,
        exchange: &OscoreExchange,
        inner: &CoapMessage,
    ) -> Result<CoapMessage, String> {
        let sender_id = self.settings.sender_id.clone();
        self.with_peer(&exchange.kid, |peer| {
            let piv = encode_piv(peer.notify_seq);
            peer.notify_seq += 1;
            protect_response_message(
                &peer.ctx,
                &sender_id,
                &exchange.kid,
                &exchange.request_piv,
                Some(&piv),
                inner,
            )
            .map_err(|_| "oscore protect notify".to_string())
        })?
    }
}

/// Encode a Partial IV: the sequence number big-endian with leading zero
/// octets stripped; 0 encodes as one zero octet (RFC 8613 §6.1 / Appendix C.8).
#[cfg(feature = "oscore")]
fn encode_piv(seq: u64) -> Vec<u8> {
    let be = seq.to_be_bytes();
    let first = be.iter().position(|&b| b != 0).unwrap_or(be.len() - 1);
    be[first..].to_vec()
}

/// Stub without the `oscore` feature: the daemon fails closed on
/// `oscore.enabled: true` (see `server::start`), so this is never constructed.
#[cfg(not(feature = "oscore"))]
pub struct OscoreGateway {
    _private: (),
}

#[cfg(not(feature = "oscore"))]
impl OscoreGateway {
    /// Unavailable without the `oscore` feature.
    ///
    /// # Errors
    /// Always.
    #[allow(clippy::missing_const_for_fn)]
    pub fn unavailable(_settings: OscoreSettings) -> Result<Self, String> {
        Err("this build has no `oscore` feature".to_string())
    }
}

#[cfg(all(test, feature = "oscore"))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::message::{CoapCode, MessageType};
    use crate::option::CoapOption;
    use crate::oscore::message::protect_request_message;

    fn settings() -> OscoreSettings {
        OscoreSettings {
            enabled: true,
            master_secret: vec![
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
                0x0f, 0x10,
            ],
            master_salt: vec![0x9e, 0x7c, 0xa9, 0x22, 0x23, 0x78, 0x63, 0x40],
            id_context: None,
            sender_id: vec![0x01],
            replay_window: 32,
        }
    }

    /// The C.1.1 client context: Sender ID empty, Recipient ID = the daemon's.
    fn client_ctx() -> SecurityContext {
        let s = settings();
        SecurityContext::derive(
            &s.master_secret,
            &s.master_salt,
            &[],
            &s.sender_id,
            None,
            AeadAlgorithm::AesCcm16_64_128,
        )
    }

    fn inner_get() -> CoapMessage {
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 7);
        m.token = vec![0x42];
        m.options.push(CoapOption::uri_path("trade"));
        m
    }

    #[test]
    fn round_trip_and_replay_rejection() {
        let gw = OscoreGateway::new(settings());
        let outer =
            protect_request_message(&client_ctx(), &[], None, &[0x14], &inner_get()).unwrap();
        let (inner, ex) = gw.unprotect_request(&outer).unwrap().unwrap();
        assert_eq!(inner.code, CoapCode::GET);
        assert_eq!(ex.request_piv, vec![0x14]);
        // Same Partial IV again — replay.
        assert_eq!(
            gw.unprotect_request(&outer).unwrap().unwrap_err(),
            OscoreReject::Replay
        );
    }

    #[test]
    fn unprotected_request_is_flagged() {
        let gw = OscoreGateway::new(settings());
        assert_eq!(
            gw.unprotect_request(&inner_get()).unwrap().unwrap_err(),
            OscoreReject::NotProtected
        );
    }

    #[test]
    fn tampered_request_does_not_move_the_replay_window() {
        let gw = OscoreGateway::new(settings());
        let mut outer =
            protect_request_message(&client_ctx(), &[], None, &[0x20], &inner_get()).unwrap();
        let n = outer.payload.len();
        outer.payload[n - 1] ^= 0x01;
        assert_eq!(
            gw.unprotect_request(&outer).unwrap().unwrap_err(),
            OscoreReject::Verify
        );
        // The genuine message with the same Partial IV still goes through.
        let genuine =
            protect_request_message(&client_ctx(), &[], None, &[0x20], &inner_get()).unwrap();
        assert!(gw.unprotect_request(&genuine).unwrap().is_ok());
    }

    #[test]
    fn response_protection_round_trips_to_the_client() {
        use crate::oscore::message::unprotect_response_message;
        let gw = OscoreGateway::new(settings());
        let outer =
            protect_request_message(&client_ctx(), &[], None, &[0x15], &inner_get()).unwrap();
        let (_, ex) = gw.unprotect_request(&outer).unwrap().unwrap();
        let mut resp = CoapMessage::new(MessageType::Acknowledgement, CoapCode::CONTENT, 7);
        resp.token = vec![0x42];
        resp.payload = b"data".to_vec();
        let protected = gw.protect_response(&ex, &resp).unwrap();
        let back =
            unprotect_response_message(&client_ctx(), &[0x01], &[], &[0x15], &protected).unwrap();
        assert_eq!(back.payload, b"data");
    }

    #[test]
    fn notification_pivs_are_unique_per_peer() {
        use crate::oscore::message::unprotect_response_message;
        let gw = OscoreGateway::new(settings());
        let outer =
            protect_request_message(&client_ctx(), &[], None, &[0x16], &inner_get()).unwrap();
        let (_, ex) = gw.unprotect_request(&outer).unwrap().unwrap();
        let mut note = CoapMessage::new(MessageType::NonConfirmable, CoapCode::CONTENT, 8);
        note.token = vec![0x42];
        note.payload = b"n".to_vec();
        let n1 = gw.protect_notification(&ex, &note).unwrap();
        let n2 = gw.protect_notification(&ex, &note).unwrap();
        let opt = |m: &CoapMessage| {
            m.options
                .iter()
                .find(|o| o.number == OSCORE_OPTION)
                .unwrap()
                .value
                .to_wire_bytes()
        };
        assert_ne!(opt(&n1), opt(&n2), "distinct Partial IVs");
        // Both decode on the client with the responder's Sender ID + own PIV.
        let b1 = unprotect_response_message(&client_ctx(), &[0x01], &[], &[0x16], &n1).unwrap();
        let b2 = unprotect_response_message(&client_ctx(), &[0x01], &[], &[0x16], &n2).unwrap();
        assert_eq!(b1.payload, b"n");
        assert_eq!(b2.payload, b"n");
    }

    #[test]
    fn piv_encoding_is_minimal_big_endian() {
        assert_eq!(encode_piv(0), vec![0x00]);
        assert_eq!(encode_piv(0x14), vec![0x14]);
        assert_eq!(encode_piv(0x0100), vec![0x01, 0x00]);
    }
}
