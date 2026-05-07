// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SampleInfo` — Metadaten pro Sample, die `DataReader::read`/`take`
//! mit jedem Sample mitliefern.
//!
//! Spec-Referenz: OMG DDS-DCPS 1.4 §2.2.2.5.1 `SampleInfo`. Die Spec
//! definiert 11 Felder, die zusammen das **Statechart** eines Samples
//! beschreiben:
//!
//! 1. `sample_state`: ob der Reader das Sample schon einmal gelesen hat
//!    (`READ`) oder nicht (`NOT_READ`).
//! 2. `view_state`: ob die Reader-Instanz neu ist (`NEW`) oder schon
//!    bekannt (`NOT_NEW`).
//! 3. `instance_state`: Lifecycle-Zustand der Instanz (`ALIVE`,
//!    `NOT_ALIVE_DISPOSED`, `NOT_ALIVE_NO_WRITERS`).
//! 4. `disposed_generation_count`: Anzahl der Uebergaenge
//!    `NOT_ALIVE_DISPOSED → ALIVE` seit dem ersten Sample dieser Instanz.
//! 5. `no_writers_generation_count`: Anzahl der Uebergaenge
//!    `NOT_ALIVE_NO_WRITERS → ALIVE` seit dem ersten Sample dieser Instanz.
//! 6. `sample_rank`: Anzahl Samples in derselben Instanz, die nach
//!    diesem im Cache stehen (Spec §2.2.2.5.1.5).
//! 7. `generation_rank`: Differenz der Generation-Counts zwischen diesem
//!    und dem letzten Sample der Instanz im selben Read-Set.
//! 8. `absolute_generation_rank`: wie `generation_rank`, aber relativ
//!    zum **aktuellen** Generation-Count.
//! 9. `source_timestamp`: Wall-Clock-Zeitpunkt der Schreiboperation.
//! 10. `instance_handle`: lokaler Handle der Instanz (Key-basiert).
//! 11. `publication_handle`: lokaler Handle des sendenden DataWriters.
//! 12. `valid_data`: `false` fuer Dispose-/Unregister-Markers ohne
//!     Nutzdaten (Spec §2.2.2.5.1.13).
//!
//! Hinweis: die Spec listet `valid_data` als 12. Feld; einige Texte
//! zaehlen es nicht mit, daher findet man sowohl "11" als auch "12"
//! Felder in der Doku. Wir tragen alle.

extern crate alloc;

use crate::instance_handle::{HANDLE_NIL, InstanceHandle};
use crate::time::Time;

/// `SampleStateKind` (DDS 1.4 §2.2.2.5.1.1) — pro Reader gepflegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SampleStateKind {
    /// Sample wurde noch nie via `read`/`take` an die Application
    /// uebergeben.
    NotRead,
    /// Sample wurde schon mindestens einmal via `read` ausgeliefert
    /// (nach `take` ist es weg, daher relevant nur fuer `read`).
    Read,
}

impl SampleStateKind {
    /// `true` wenn dies ein noch ungelesenes Sample ist.
    #[must_use]
    pub const fn is_not_read(&self) -> bool {
        matches!(self, Self::NotRead)
    }
}

impl Default for SampleStateKind {
    fn default() -> Self {
        Self::NotRead
    }
}

/// `ViewStateKind` (DDS 1.4 §2.2.2.5.1.2) — pro Instanz gepflegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ViewStateKind {
    /// Erstes Sample dieser Instanz (oder erstes nach
    /// `NOT_ALIVE_NO_WRITERS → ALIVE`).
    New,
    /// Reader hat fuer diese Instanz schon Samples ausgeliefert.
    NotNew,
}

impl Default for ViewStateKind {
    fn default() -> Self {
        Self::New
    }
}

/// `InstanceStateKind` (DDS 1.4 §2.2.2.5.1.3) — pro Instanz gepflegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceStateKind {
    /// Mindestens ein DataWriter hat die Instanz registriert und sie
    /// wurde nicht disposed.
    Alive,
    /// Mindestens ein DataWriter hat die Instanz disposed.
    NotAliveDisposed,
    /// Alle DataWriter, die die Instanz registriert hatten, haben
    /// `unregister_instance` aufgerufen oder sind weg.
    NotAliveNoWriters,
}

impl InstanceStateKind {
    /// `true` wenn die Instanz noch lebt.
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        matches!(self, Self::Alive)
    }
    /// `true` wenn die Instanz entweder disposed oder no-writers ist.
    #[must_use]
    pub const fn is_not_alive(&self) -> bool {
        !self.is_alive()
    }
}

impl Default for InstanceStateKind {
    fn default() -> Self {
        Self::Alive
    }
}

/// Bitmask-Maske fuer Sample-State-Filter in `read`/`take`-Calls
/// (DDS 1.4 §2.2.2.5.1.4 `SampleStateMask`).
pub mod sample_state_mask {
    /// `NOT_READ` only.
    pub const NOT_READ: u32 = 1 << 0;
    /// `READ` only.
    pub const READ: u32 = 1 << 1;
    /// `READ | NOT_READ` — beide.
    pub const ANY: u32 = NOT_READ | READ;
}

/// Bitmask-Maske fuer View-State-Filter (§2.2.2.5.1.4 `ViewStateMask`).
pub mod view_state_mask {
    /// `NEW`.
    pub const NEW: u32 = 1 << 0;
    /// `NOT_NEW`.
    pub const NOT_NEW: u32 = 1 << 1;
    /// Beide.
    pub const ANY: u32 = NEW | NOT_NEW;
}

/// Bitmask-Maske fuer Instance-State-Filter (§2.2.2.5.1.4
/// `InstanceStateMask`).
pub mod instance_state_mask {
    /// `ALIVE`.
    pub const ALIVE: u32 = 1 << 0;
    /// `NOT_ALIVE_DISPOSED`.
    pub const NOT_ALIVE_DISPOSED: u32 = 1 << 1;
    /// `NOT_ALIVE_NO_WRITERS`.
    pub const NOT_ALIVE_NO_WRITERS: u32 = 1 << 2;
    /// `NOT_ALIVE_DISPOSED | NOT_ALIVE_NO_WRITERS`.
    pub const NOT_ALIVE: u32 = NOT_ALIVE_DISPOSED | NOT_ALIVE_NO_WRITERS;
    /// Alle drei.
    pub const ANY: u32 = ALIVE | NOT_ALIVE;
}

/// `SampleInfo` (DDS 1.4 §2.2.2.5.1) — Metadaten pro Sample.
///
/// Wird vom DataReader im `take`/`read`-Pfad gefuellt. Application-
/// Code soll sich auf diese Felder verlassen, um:
/// * neue Samples vs. wiederholt gelesene zu unterscheiden
///   (`sample_state`),
/// * neue Instanzen zu erkennen (`view_state == NEW`),
/// * Lifecycle-Events zu sehen (`instance_state`, `valid_data == false`),
/// * Reordering / Generations-Wechsel zu folgen (`*_generation_count`,
///   `*_rank`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleInfo {
    /// Sample-State (`READ` / `NOT_READ`).
    pub sample_state: SampleStateKind,
    /// View-State (`NEW` / `NOT_NEW`).
    pub view_state: ViewStateKind,
    /// Instance-State (`ALIVE` / `NOT_ALIVE_*`).
    pub instance_state: InstanceStateKind,
    /// Wieviele Mal die Instanz `NOT_ALIVE_DISPOSED → ALIVE` durchlaufen
    /// hat seit ihrem ersten Sample.
    pub disposed_generation_count: i32,
    /// Wieviele Mal die Instanz `NOT_ALIVE_NO_WRITERS → ALIVE`
    /// durchlaufen hat seit ihrem ersten Sample.
    pub no_writers_generation_count: i32,
    /// Anzahl Samples in derselben Instanz, die nach diesem im Cache
    /// stehen (§2.2.2.5.1.5).
    pub sample_rank: i32,
    /// Differenz `generation_count` zwischen diesem und dem letzten
    /// Sample der Instanz im selben Read-Set.
    pub generation_rank: i32,
    /// Differenz `generation_count` zwischen diesem und dem **aktuellen**
    /// Generation-Count.
    pub absolute_generation_rank: i32,
    /// Wall-Clock-Zeitpunkt des `write`/`dispose`/`unregister`.
    pub source_timestamp: Time,
    /// Lokaler Handle der Instanz.
    pub instance_handle: InstanceHandle,
    /// Lokaler Handle des sendenden DataWriters (im offline / unbekannten
    /// Fall `HANDLE_NIL`).
    pub publication_handle: InstanceHandle,
    /// `true` wenn das Sample Nutzdaten enthaelt; `false` bei
    /// reinem Dispose-/Unregister-Marker (§2.2.2.5.1.13).
    pub valid_data: bool,
}

impl Default for SampleInfo {
    fn default() -> Self {
        Self {
            sample_state: SampleStateKind::NotRead,
            view_state: ViewStateKind::New,
            instance_state: InstanceStateKind::Alive,
            disposed_generation_count: 0,
            no_writers_generation_count: 0,
            sample_rank: 0,
            generation_rank: 0,
            absolute_generation_rank: 0,
            source_timestamp: Time::default(),
            instance_handle: HANDLE_NIL,
            publication_handle: HANDLE_NIL,
            valid_data: true,
        }
    }
}

impl SampleInfo {
    /// Konstruiert einen Default-Info-Block fuer ein frisches Sample
    /// einer **neuen** Instanz mit `valid_data = true`.
    #[must_use]
    pub fn new_alive(
        instance: InstanceHandle,
        publication: InstanceHandle,
        timestamp: Time,
    ) -> Self {
        Self {
            sample_state: SampleStateKind::NotRead,
            view_state: ViewStateKind::New,
            instance_state: InstanceStateKind::Alive,
            instance_handle: instance,
            publication_handle: publication,
            source_timestamp: timestamp,
            valid_data: true,
            ..Self::default()
        }
    }

    /// Prueft, ob dieses `SampleInfo` durch die uebergebenen
    /// State-Masks akzeptiert wird (Spec §2.2.2.5.3.1, `read_w_condition`).
    #[must_use]
    pub fn matches_states(&self, sample_mask: u32, view_mask: u32, instance_mask: u32) -> bool {
        let sample_bit = match self.sample_state {
            SampleStateKind::NotRead => sample_state_mask::NOT_READ,
            SampleStateKind::Read => sample_state_mask::READ,
        };
        let view_bit = match self.view_state {
            ViewStateKind::New => view_state_mask::NEW,
            ViewStateKind::NotNew => view_state_mask::NOT_NEW,
        };
        let inst_bit = match self.instance_state {
            InstanceStateKind::Alive => instance_state_mask::ALIVE,
            InstanceStateKind::NotAliveDisposed => instance_state_mask::NOT_ALIVE_DISPOSED,
            InstanceStateKind::NotAliveNoWriters => instance_state_mask::NOT_ALIVE_NO_WRITERS,
        };
        (sample_mask & sample_bit) != 0
            && (view_mask & view_bit) != 0
            && (instance_mask & inst_bit) != 0
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_alive_new_not_read() {
        let info = SampleInfo::default();
        assert_eq!(info.sample_state, SampleStateKind::NotRead);
        assert_eq!(info.view_state, ViewStateKind::New);
        assert_eq!(info.instance_state, InstanceStateKind::Alive);
        assert!(info.valid_data);
        assert_eq!(info.disposed_generation_count, 0);
        assert_eq!(info.no_writers_generation_count, 0);
        assert_eq!(info.instance_handle, HANDLE_NIL);
        assert_eq!(info.publication_handle, HANDLE_NIL);
    }

    #[test]
    fn instance_state_predicates() {
        assert!(InstanceStateKind::Alive.is_alive());
        assert!(!InstanceStateKind::Alive.is_not_alive());
        assert!(InstanceStateKind::NotAliveDisposed.is_not_alive());
        assert!(InstanceStateKind::NotAliveNoWriters.is_not_alive());
    }

    #[test]
    fn sample_state_predicate() {
        assert!(SampleStateKind::NotRead.is_not_read());
        assert!(!SampleStateKind::Read.is_not_read());
    }

    #[test]
    fn matches_states_filter() {
        let info = SampleInfo::default();
        assert!(info.matches_states(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        ));
        assert!(info.matches_states(
            sample_state_mask::NOT_READ,
            view_state_mask::NEW,
            instance_state_mask::ALIVE,
        ));
        assert!(!info.matches_states(
            sample_state_mask::READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        ));
        assert!(!info.matches_states(
            sample_state_mask::ANY,
            view_state_mask::NOT_NEW,
            instance_state_mask::ANY,
        ));
        assert!(!info.matches_states(
            sample_state_mask::ANY,
            view_state_mask::ANY,
            instance_state_mask::NOT_ALIVE,
        ));
    }

    #[test]
    fn new_alive_constructor_sets_handles_and_timestamp() {
        let h = InstanceHandle::from_raw(7);
        let pub_h = InstanceHandle::from_raw(42);
        let ts = Time::new(1, 2);
        let info = SampleInfo::new_alive(h, pub_h, ts);
        assert_eq!(info.instance_handle, h);
        assert_eq!(info.publication_handle, pub_h);
        assert_eq!(info.source_timestamp, ts);
        assert!(info.valid_data);
        assert_eq!(info.instance_state, InstanceStateKind::Alive);
    }

    // ---- §2.2.2.5.5 alle 12 SampleInfo-Felder verfuegbar ----

    #[test]
    fn sample_info_all_spec_fields_accessible() {
        // Spec §2.2.2.5.5 listet 12 Felder; alle muessen lesbar +
        // setzbar sein. Test ueberprueft die Feld-Identitaet via
        // konkrete Werte.
        let info = SampleInfo {
            sample_state: SampleStateKind::Read,
            view_state: ViewStateKind::NotNew,
            instance_state: InstanceStateKind::NotAliveDisposed,
            disposed_generation_count: 3,
            no_writers_generation_count: 5,
            sample_rank: 7,
            generation_rank: 9,
            absolute_generation_rank: 11,
            source_timestamp: Time::new(1, 2),
            instance_handle: InstanceHandle::from_raw(0xCAFE),
            publication_handle: InstanceHandle::from_raw(0xBEEF),
            valid_data: false,
        };

        assert_eq!(info.sample_state, SampleStateKind::Read);
        assert_eq!(info.view_state, ViewStateKind::NotNew);
        assert_eq!(info.instance_state, InstanceStateKind::NotAliveDisposed);
        assert_eq!(info.disposed_generation_count, 3);
        assert_eq!(info.no_writers_generation_count, 5);
        assert_eq!(info.sample_rank, 7);
        assert_eq!(info.generation_rank, 9);
        assert_eq!(info.absolute_generation_rank, 11);
        assert_eq!(info.source_timestamp, Time::new(1, 2));
        assert_eq!(info.instance_handle, InstanceHandle::from_raw(0xCAFE));
        assert_eq!(info.publication_handle, InstanceHandle::from_raw(0xBEEF));
        assert!(!info.valid_data);
    }

    #[test]
    fn sample_info_dispose_marker_has_invalid_data() {
        // §2.2.2.5.1.13 — Dispose-/Unregister-Marker hat valid_data=false.
        // Negativ: ein Sample mit valid_data=false hat keine Nutzdaten,
        // der Caller MUSS sample.data nicht auswerten.
        let info = SampleInfo {
            valid_data: false,
            instance_state: InstanceStateKind::NotAliveDisposed,
            ..SampleInfo::default()
        };
        assert!(!info.valid_data);
        assert!(info.instance_state.is_not_alive());
    }

    #[test]
    fn sample_info_three_state_dimensions_independent() {
        // Spec §2.2.2.5.1 — drei orthogonale State-Dimensionen.
        // matches_states muss auf jede einzeln filtern.
        let info = SampleInfo {
            sample_state: SampleStateKind::Read,
            view_state: ViewStateKind::NotNew,
            instance_state: InstanceStateKind::Alive,
            ..SampleInfo::default()
        };
        // Read+NotNew+Alive matched.
        assert!(info.matches_states(
            sample_state_mask::READ,
            view_state_mask::NOT_NEW,
            instance_state_mask::ALIVE,
        ));
        // sample_state mismatch → reject.
        assert!(!info.matches_states(
            sample_state_mask::NOT_READ,
            view_state_mask::ANY,
            instance_state_mask::ANY,
        ));
    }

    #[test]
    fn sample_info_generation_rank_starts_zero() {
        // Default-Sample: alle Generation-Counter 0.
        let info = SampleInfo::default();
        assert_eq!(info.disposed_generation_count, 0);
        assert_eq!(info.no_writers_generation_count, 0);
        assert_eq!(info.sample_rank, 0);
        assert_eq!(info.generation_rank, 0);
        assert_eq!(info.absolute_generation_rank, 0);
    }

    #[test]
    fn enum_default_impls() {
        assert_eq!(SampleStateKind::default(), SampleStateKind::NotRead);
        assert_eq!(ViewStateKind::default(), ViewStateKind::New);
        assert_eq!(InstanceStateKind::default(), InstanceStateKind::Alive);
    }
}
