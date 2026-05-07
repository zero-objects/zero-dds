// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Exclusive-Ownership-Resolution (DDS 1.4 §2.2.3.23 / §2.2.2.5.5).
//!
//! Wenn ein Topic mit `Exclusive`-Ownership-QoS konfiguriert ist und
//! mehrere DataWriter die selbe Instanz schreiben, MUSS der DataReader
//! Samples nur vom **aktuell strongest writer** ausliefern. Der "strongest
//! writer" ist der mit dem **hoechsten ownership_strength**-Wert; bei
//! Gleichstand entscheidet die GUID lexikografisch (Tie-Breaker).
//!
//! Dieses Modul liefert die zustandslose Resolver-Funktion. Der Caller
//! (DataReader.take()) muss pro Instanz einen `OwnershipResolver`-State
//! halten.
//!
//! WP 2.8 (C2.8 — QoS-Vollstaendigkeit).

extern crate alloc;

use alloc::vec::Vec;

/// 16-byte GUID-Reference (eines Writers). Wir kopieren die GUID als
/// `[u8; 16]` rein, damit das Modul kein zerodds-rtps-Dependency braucht.
pub type WriterGuidBytes = [u8; 16];

/// Ein einzelner Kandidaten-Writer fuer Exclusive-Ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipCandidate {
    /// 16-byte Writer-GUID.
    pub guid: WriterGuidBytes,
    /// `OwnershipStrengthQosPolicy.value` (i32, default 0).
    pub strength: i32,
    /// True wenn der Writer aktuell als ALIVE bekannt ist (Liveliness-State).
    /// Inaktive Writer werden bei der Resolution ausgeschlossen.
    pub alive: bool,
}

/// Wählt aus einer Kandidatenliste den aktuellen "strongest writer"
/// gemäß DDS 1.4 §2.2.3.23.4:
///
/// 1. Filtere nicht-alive Writer.
/// 2. Sortiere nach `strength` absteigend; bei Gleichstand nach GUID
///    aufsteigend (Tie-Breaker).
/// 3. Liefere den ersten Eintrag (oder `None` wenn alle gefiltert wurden).
#[must_use]
pub fn resolve_strongest(candidates: &[OwnershipCandidate]) -> Option<OwnershipCandidate> {
    let mut alive: Vec<OwnershipCandidate> =
        candidates.iter().copied().filter(|c| c.alive).collect();
    if alive.is_empty() {
        return None;
    }
    // Sort: höchste strength zuerst; bei Gleichstand kleinste GUID zuerst.
    alive.sort_by(|a, b| {
        b.strength
            .cmp(&a.strength)
            .then_with(|| a.guid.cmp(&b.guid))
    });
    alive.first().copied()
}

/// Stateful-Resolver — wird pro Instanz im DataReader gehalten und
/// erinnert den aktuell akzeptierten Writer.
///
/// `accept(sample_writer)` liefert `true` wenn das Sample ausgeliefert
/// werden soll, `false` wenn gefiltert werden soll.
#[derive(Debug, Clone, Default)]
pub struct OwnershipResolver {
    /// Bekannte Writer mit ihrem aktuellen Strength + Alive-State.
    candidates: Vec<OwnershipCandidate>,
    /// Cache des aktuell strongest Writers.
    current: Option<WriterGuidBytes>,
}

impl OwnershipResolver {
    /// Neuer leerer Resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registriert einen Writer (oder aktualisiert dessen Strength/Alive).
    /// Recomputed den aktuell strongest writer.
    pub fn upsert(&mut self, candidate: OwnershipCandidate) {
        if let Some(existing) = self
            .candidates
            .iter_mut()
            .find(|c| c.guid == candidate.guid)
        {
            *existing = candidate;
        } else {
            self.candidates.push(candidate);
        }
        self.recompute();
    }

    /// Entfernt einen Writer (z.B. nach unmatch oder lost-Liveliness).
    pub fn remove(&mut self, guid: &WriterGuidBytes) {
        self.candidates.retain(|c| &c.guid != guid);
        self.recompute();
    }

    /// True wenn der Writer aktuell der strongest ist.
    #[must_use]
    pub fn is_strongest(&self, guid: &WriterGuidBytes) -> bool {
        self.current.as_ref() == Some(guid)
    }

    /// True wenn ein Sample dieses Writers ausgeliefert werden soll.
    /// Convenience-Wrapper um `is_strongest`.
    #[must_use]
    pub fn accept(&self, writer_guid: &WriterGuidBytes) -> bool {
        self.is_strongest(writer_guid)
    }

    /// Aktueller strongest Writer.
    #[must_use]
    pub fn current(&self) -> Option<WriterGuidBytes> {
        self.current
    }

    fn recompute(&mut self) {
        self.current = resolve_strongest(&self.candidates).map(|c| c.guid);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn cand(byte: u8, strength: i32, alive: bool) -> OwnershipCandidate {
        OwnershipCandidate {
            guid: [byte; 16],
            strength,
            alive,
        }
    }

    #[test]
    fn resolve_picks_highest_strength() {
        let cs = [
            cand(0xAA, 5, true),
            cand(0xBB, 10, true),
            cand(0xCC, 1, true),
        ];
        assert_eq!(resolve_strongest(&cs).unwrap().guid, [0xBB; 16]);
    }

    #[test]
    fn resolve_ignores_non_alive() {
        let cs = [
            cand(0xAA, 100, false), // hoechste strength aber nicht alive
            cand(0xBB, 5, true),
        ];
        assert_eq!(resolve_strongest(&cs).unwrap().guid, [0xBB; 16]);
    }

    #[test]
    fn resolve_returns_none_if_all_dead() {
        let cs = [cand(0xAA, 10, false), cand(0xBB, 5, false)];
        assert!(resolve_strongest(&cs).is_none());
    }

    #[test]
    fn tie_breaker_uses_smallest_guid() {
        // Beide haben strength=5; kleinere GUID gewinnt.
        let cs = [cand(0xBB, 5, true), cand(0xAA, 5, true)];
        assert_eq!(resolve_strongest(&cs).unwrap().guid, [0xAA; 16]);
    }

    #[test]
    fn resolver_upsert_tracks_strongest() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 5, true));
        r.upsert(cand(0xBB, 10, true));
        assert!(r.is_strongest(&[0xBB; 16]));
        assert!(!r.is_strongest(&[0xAA; 16]));
    }

    #[test]
    fn resolver_strength_change_triggers_swap() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 5, true));
        r.upsert(cand(0xBB, 10, true));
        assert!(r.is_strongest(&[0xBB; 16]));
        // BB sinkt unter AA
        r.upsert(cand(0xBB, 1, true));
        assert!(r.is_strongest(&[0xAA; 16]));
    }

    #[test]
    fn resolver_remove_triggers_recompute() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 5, true));
        r.upsert(cand(0xBB, 10, true));
        assert!(r.is_strongest(&[0xBB; 16]));
        r.remove(&[0xBB; 16]);
        assert!(r.is_strongest(&[0xAA; 16]));
    }

    #[test]
    fn resolver_dead_strongest_falls_through() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 10, true));
        r.upsert(cand(0xBB, 5, true));
        assert!(r.is_strongest(&[0xAA; 16]));
        // AA wird ALIVE→NOT_ALIVE
        r.upsert(cand(0xAA, 10, false));
        assert!(r.is_strongest(&[0xBB; 16]));
    }

    #[test]
    fn resolver_accept_helper() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 10, true));
        assert!(r.accept(&[0xAA; 16]));
        assert!(!r.accept(&[0xBB; 16]));
    }

    #[test]
    fn empty_resolver_accepts_nothing() {
        let r = OwnershipResolver::new();
        assert!(!r.accept(&[0xAA; 16]));
        assert!(r.current().is_none());
    }
}
