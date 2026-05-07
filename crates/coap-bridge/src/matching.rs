// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request/Response-Matching nach RFC 7252 §5.3.
//!
//! CoAP nutzt Tokens, um asynchrone Antworten Anfragen zuzuordnen.
//! Wir kapseln die Pending-Request-Map als wiederverwendbaren
//! Helper.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::time::Duration;
use std::sync::Mutex;
use std::time::Instant;

use crate::message::CoapMessage;

/// Pending-Request-Eintrag.
struct Pending {
    request: CoapMessage,
    deadline: Instant,
}

impl core::fmt::Debug for Pending {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pending")
            .field("token_len", &self.request.token.len())
            .finish()
    }
}

/// Pending-Request-Map nach RFC 7252 §5.3.
///
/// Ermoeglicht den Lookup einer Request anhand des Tokens beim
/// Eintreffen einer Response. Trackt einen Timeout pro Eintrag
/// (Spec §4.8 EXCHANGE_LIFETIME = 247s default).
#[derive(Default)]
pub struct PendingRequests {
    by_token: Mutex<BTreeMap<Vec<u8>, Pending>>,
    /// DoS-Cap fuer simultaneous pending requests.
    pub max_pending: usize,
}

impl core::fmt::Debug for PendingRequests {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let n = self.by_token.lock().map_or(0, |g| g.len());
        f.debug_struct("PendingRequests")
            .field("count", &n)
            .field("max_pending", &self.max_pending)
            .finish()
    }
}

/// Errors beim Pending-Request-Tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchError {
    /// Token ist leer (Spec verlangt nicht-leeres Token fuer Async).
    EmptyToken,
    /// Cap ueberschritten.
    PendingCapReached,
    /// Duplicate Token (gleicher Request bereits eingetragen).
    DuplicateToken,
}

impl PendingRequests {
    /// Konstruktor mit DoS-Cap.
    #[must_use]
    pub fn new(max_pending: usize) -> Self {
        Self {
            by_token: Mutex::new(BTreeMap::new()),
            max_pending,
        }
    }

    /// Spec §5.3.1 — registriert eine Pending Request.
    ///
    /// # Errors
    /// `EmptyToken` wenn `request.token` leer ist;
    /// `PendingCapReached` wenn `max_pending` erreicht;
    /// `DuplicateToken` wenn Token bereits eingetragen.
    pub fn submit(
        &self,
        request: CoapMessage,
        exchange_lifetime: Duration,
    ) -> Result<(), MatchError> {
        if request.token.is_empty() {
            return Err(MatchError::EmptyToken);
        }
        let mut g = self
            .by_token
            .lock()
            .map_err(|_| MatchError::PendingCapReached)?;
        if g.len() >= self.max_pending {
            return Err(MatchError::PendingCapReached);
        }
        if g.contains_key(&request.token) {
            return Err(MatchError::DuplicateToken);
        }
        let token = request.token.clone();
        g.insert(
            token,
            Pending {
                request,
                deadline: Instant::now() + exchange_lifetime,
            },
        );
        Ok(())
    }

    /// Spec §5.3.2 — Match auf Response: liefert die zugehoerige
    /// Request (und entfernt den Pending-Eintrag), oder `None`.
    pub fn complete(&self, response_token: &[u8]) -> Option<CoapMessage> {
        let mut g = self.by_token.lock().ok()?;
        g.remove(response_token).map(|p| p.request)
    }

    /// Anzahl pending Requests.
    pub fn pending_count(&self) -> usize {
        self.by_token.lock().map_or(0, |g| g.len())
    }

    /// Spec §4.8.2 — entfernt abgelaufene Pendings (deadline < now).
    /// Liefert die Anzahl evicted Eintraege.
    pub fn evict_expired(&self) -> usize {
        let now = Instant::now();
        if let Ok(mut g) = self.by_token.lock() {
            let before = g.len();
            g.retain(|_, p| p.deadline > now);
            before - g.len()
        } else {
            0
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::message::{CoapCode, MessageType};

    fn req_with_token(token: Vec<u8>) -> CoapMessage {
        let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 1);
        m.token = token;
        m
    }

    #[test]
    fn submit_empty_token_rejected() {
        let p = PendingRequests::new(10);
        assert_eq!(
            p.submit(req_with_token(alloc::vec![]), Duration::from_secs(60)),
            Err(MatchError::EmptyToken)
        );
    }

    #[test]
    fn submit_then_complete_round_trip() {
        let p = PendingRequests::new(10);
        p.submit(
            req_with_token(alloc::vec![1, 2, 3]),
            Duration::from_secs(60),
        )
        .expect("ok");
        assert_eq!(p.pending_count(), 1);
        let r = p.complete(&[1, 2, 3]).expect("matched");
        assert_eq!(r.token, alloc::vec![1, 2, 3]);
        assert_eq!(p.pending_count(), 0);
    }

    #[test]
    fn complete_unknown_token_returns_none() {
        let p = PendingRequests::new(10);
        assert!(p.complete(&[9, 9]).is_none());
    }

    #[test]
    fn submit_above_cap_rejected() {
        let p = PendingRequests::new(2);
        p.submit(req_with_token(alloc::vec![1]), Duration::from_secs(60))
            .expect("ok");
        p.submit(req_with_token(alloc::vec![2]), Duration::from_secs(60))
            .expect("ok");
        assert_eq!(
            p.submit(req_with_token(alloc::vec![3]), Duration::from_secs(60)),
            Err(MatchError::PendingCapReached)
        );
    }

    #[test]
    fn submit_duplicate_token_rejected() {
        let p = PendingRequests::new(10);
        p.submit(req_with_token(alloc::vec![1, 2]), Duration::from_secs(60))
            .expect("ok");
        assert_eq!(
            p.submit(req_with_token(alloc::vec![1, 2]), Duration::from_secs(60)),
            Err(MatchError::DuplicateToken)
        );
    }

    #[test]
    fn evict_expired_removes_overdue_pendings() {
        let p = PendingRequests::new(10);
        p.submit(req_with_token(alloc::vec![1]), Duration::from_millis(1))
            .expect("ok");
        std::thread::sleep(Duration::from_millis(10));
        let evicted = p.evict_expired();
        assert_eq!(evicted, 1);
        assert_eq!(p.pending_count(), 0);
    }

    #[test]
    fn evict_keeps_fresh_pendings() {
        let p = PendingRequests::new(10);
        p.submit(req_with_token(alloc::vec![1]), Duration::from_secs(60))
            .expect("ok");
        let evicted = p.evict_expired();
        assert_eq!(evicted, 0);
        assert_eq!(p.pending_count(), 1);
    }
}
