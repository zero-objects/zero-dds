// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Flatdata-Integration (ADR-0005, opt-in via Feature
//! `flatdata-integration`).
//!
//! Bietet `DataWriter::write_flat` und `DataReader::read_flat` als
//! conditional Methoden mit `T: FlatStruct`-Bound. Default-Build
//! sieht diese Methoden nicht — Caller muss explizit
//! `--features flatdata-integration` aktivieren.
//!
//! # Architektur
//!
//! Spec `zerodds-flatdata-1.0` §8/§9 verlangt die spec-konformen
//! Methoden direkt am `DataWriter<T>` / `DataReader<T>`:
//!
//! 1. `DataWriter::set_flat_backend(backend, mask)` und
//!    `DataReader::set_flat_backend(backend, reader_index)` koppeln
//!    den Slot-Backend an die Entity. Konfiguration einmalig nach
//!    Discovery-Match.
//! 2. `DataWriter::write_flat(sample)` schreibt in den SHM-Slot
//!    (Spec §4.1) **und** sendet parallel UDP-DATA fuer Cross-Host-
//!    Reader (§4.2 Cross-Host-Fallback). Ohne konfiguriertes Backend
//!    faellt die Methode auf den klassischen UDP-Pfad zurueck.
//! 3. `DataReader::read_flat()` liefert das neueste Sample aus dem
//!    Slot-Pool (Spec §9.1) und faellt auf `take()` zurueck wenn kein
//!    Slot vorhanden ist.
//!
//! Zusaetzlich existiert die Builder-Variante `FlatWriterExt` /
//! `FlatReaderExt` (siehe weiter unten in diesem Modul) als
//! alternative API, die ohne das Setzen am Entity-Objekt auskommt —
//! nuetzlich, wenn ein Caller mehrere Slot-Backends parallel mit
//! demselben DataWriter benutzen moechte.
//!
//! # Sicherheits-Begruendung
//!
//! `from_bytes_unchecked` ist `unsafe fn`. Der Caller hier hat den
//! Type-Hash + Sample-Size verifiziert (siehe `read_flat`).
//! Lokales `#[allow(unsafe_code)]` mit per-Block-SAFETY-Kommentar.
//!
//! Spec: `docs/specs/zerodds-flatdata-1.0.md` §4 + §8 + §9.

#![allow(unsafe_code)]

use alloc::sync::Arc;
use core::marker::PhantomData;

use zerodds_flatdata::{FlatStruct, SlotBackend, SlotError, SlotHandle};

use crate::dds_type::DdsType;
use crate::error::{DdsError, Result};
use crate::publisher::DataWriter;
use crate::subscriber::DataReader;

/// Convertiert `SlotError` in `DdsError`.
fn slot_to_dds(e: SlotError) -> DdsError {
    match e {
        SlotError::NoFreeSlot => DdsError::OutOfResources {
            what: "flatdata: no free slot",
        },
        SlotError::OutOfBounds => DdsError::BadParameter {
            what: "flatdata: slot index out of bounds",
        },
        SlotError::SampleTooLarge { .. } => DdsError::OutOfResources {
            what: "flatdata: sample too large",
        },
        SlotError::LockPoisoned => DdsError::PreconditionNotMet {
            reason: "flatdata: slot lock poisoned",
        },
    }
}

// ============================================================================
// Spec-konforme Methoden direkt am DataWriter/DataReader (§8.1, §9.1)
// ============================================================================

impl<T: DdsType + FlatStruct + Send + Sync + 'static> DataWriter<T> {
    /// Setzt den Flatdata-SlotBackend fuer den Same-Host-Zero-Copy-
    /// Pfad. Bei `Some(backend)` schreibt [`Self::write_flat`] das
    /// Sample direkt in einen SHM-Slot (Spec §4.1) und sendet parallel
    /// UDP-DATA fuer Cross-Host-Reader. `active_readers_mask` listet
    /// die Reader-Bits, die fuer die Slot-Wiederverwendung relevant
    /// sind (Spec §5.1 Slot-Refcount).
    ///
    /// `None` deaktiviert den SHM-Pfad — nachfolgende `write_flat()`
    /// fallen auf den klassischen UDP-Pfad zurueck.
    ///
    /// Spec: `zerodds-flatdata-1.0` §4.1 + §8.
    pub fn set_flat_backend(
        &self,
        backend: Option<Arc<dyn SlotBackend>>,
        active_readers_mask: u32,
    ) {
        if let Ok(mut slot) = self.flat_backend.lock() {
            *slot = backend.map(|b| (b, active_readers_mask));
        }
    }

    /// Spec §8.1 `write_flat` — schreibt einen `FlatStruct`-Sample
    /// direkt in den SHM-Slot (kein CDR-Encode) und sendet parallel
    /// UDP-DATA fuer Cross-Host-Reader (Spec §4.2). Ohne
    /// konfiguriertes Backend (siehe [`Self::set_flat_backend`])
    /// faellt der Aufruf auf den klassischen UDP-Pfad zurueck.
    ///
    /// # Errors
    /// - `OutOfResources` bei vollem Slot-Pool oder
    ///   `SampleTooLarge`.
    /// - `WireError` bei CDR-Encode-Failure des UDP-Cross-Host-Pfades.
    /// - `PreconditionNotMet` bei Slot-Lock-Poisoning.
    pub fn write_flat(&self, sample: &T) -> Result<()> {
        let backend_snapshot = self
            .flat_backend
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "flatdata: backend mutex poisoned",
            })?
            .clone();

        if let Some((backend, mask)) = backend_snapshot {
            // Same-Host (SHM): write ohne CDR-Encode.
            let bytes = sample.as_bytes();
            let handle = backend.reserve_slot(mask).map_err(slot_to_dds)?;
            if let Err(e) = backend.commit_slot(handle, bytes) {
                let _ = backend.discard_slot(handle);
                return Err(slot_to_dds(e));
            }
            // Cross-Host (UDP): klassischer DCPS-Pfad parallel
            // (Spec §4.2 Cross-Host-Fallback).
            self.write(sample)
        } else {
            // Kein Backend konfiguriert → reiner UDP-Pfad.
            self.write(sample)
        }
    }
}

impl<T: DdsType + FlatStruct + Send + Sync + 'static> DataReader<T> {
    /// Setzt den Flatdata-SlotBackend fuer den Same-Host-Zero-Copy-
    /// Lese-Pfad. `reader_index` ist das Bit (0..31) im
    /// `slot.reader_mask`, das dieser Reader nach erfolgreichem Read
    /// setzt (Spec §5.1).
    ///
    /// `None` deaktiviert den SHM-Pfad — nachfolgende `read_flat()`
    /// fallen auf `take()` zurueck.
    ///
    /// Spec: `zerodds-flatdata-1.0` §4.1 + §9.
    pub fn set_flat_backend(&self, backend: Option<Arc<dyn SlotBackend>>, reader_index: u8) {
        use core::sync::atomic::AtomicU32;
        if let Ok(mut slot) = self.flat_backend.lock() {
            *slot = backend.map(|b| (b, reader_index, AtomicU32::new(u32::MAX)));
        }
    }

    /// Spec §9.1 `read_flat` — bevorzugt Same-Host-SHM, falls
    /// Backend konfiguriert ist; faellt auf [`Self::take`] zurueck
    /// wenn kein Slot vorhanden oder kein Backend gesetzt ist.
    ///
    /// Spec §6.1: prueft `backend.type_hash()` gegen `T::TYPE_HASH`;
    /// bei Mismatch liefert `Err(PreconditionNotMet)` ohne den Slot
    /// zu dereferenzieren (Schutz gegen Schema-Drift).
    ///
    /// # Errors
    /// `PreconditionNotMet` bei Type-Hash-Mismatch oder Slot-Lock-
    /// Poisoning.
    pub fn read_flat(&self) -> Result<Option<T>> {
        let mut slot = self
            .flat_backend
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "flatdata: backend mutex poisoned",
            })?;
        let Some((backend, reader_index, last_sn)) = slot.as_mut() else {
            return Ok(None);
        };
        // Spec §6.1: Type-Hash Cross-Validation.
        if let Some(backend_hash) = backend.type_hash() {
            if backend_hash != T::TYPE_HASH {
                return Err(DdsError::PreconditionNotMet {
                    reason: "flatdata: TYPE_HASH mismatch (schema drift)",
                });
            }
        }
        scan_slots::<T>(backend.as_ref(), *reader_index, last_sn)
    }
}

/// Scan-Helfer: liefert das neueste un-gelesene Sample aus dem
/// Slot-Pool (Spec §9.1). Geteilt zwischen `DataReader::read_flat`
/// und [`FlatReaderExt::read_flat`].
fn scan_slots<T: FlatStruct>(
    backend: &dyn SlotBackend,
    reader_index: u8,
    last_sn: &core::sync::atomic::AtomicU32,
) -> Result<Option<T>> {
    let count = backend.slot_count().map_err(slot_to_dds)?;
    let last_seen = last_sn.load(core::sync::atomic::Ordering::Relaxed);
    let mut best: Option<(u32, T)> = None;
    for idx in 0..count {
        let handle = SlotHandle {
            segment_id: 0,
            slot_index: idx as u32,
        };
        let (header, bytes) = backend.read_slot(handle).map_err(slot_to_dds)?;
        if header.sample_size == 0 {
            continue;
        }
        if (header.reader_mask & (1u32 << reader_index)) != 0 {
            continue;
        }
        if (bytes.len() as u32) < T::WIRE_SIZE as u32 {
            continue;
        }
        // SAFETY: WIRE_SIZE gepruft; FlatStruct-Bound garantiert
        // Layout-Consistency.
        let sample = unsafe { T::from_bytes_unchecked(&bytes) };
        let unseen = last_seen == u32::MAX || header.sequence_number > last_seen;
        let beats_current = best
            .as_ref()
            .is_none_or(|(b_sn, _)| header.sequence_number > *b_sn);
        if unseen && beats_current {
            best = Some((header.sequence_number, sample));
        }
        backend
            .mark_read(handle, reader_index)
            .map_err(slot_to_dds)?;
    }
    if let Some((sn, _)) = best.as_ref() {
        last_sn.store(*sn, core::sync::atomic::Ordering::Relaxed);
    }
    Ok(best.map(|(_, t)| t))
}

// ============================================================================
// Builder-API (alternative, fuer Caller die mehrere Backends parallel
// mit derselben Entity nutzen wollen)
// ============================================================================

/// Builder-API fuer Flatdata-Schreibpfade. Alternative zur
/// [`DataWriter::set_flat_backend`]-Methode: kapselt einen Slot-Backend
/// und exponiert `write_flat` ohne State an der Entity.
pub struct FlatWriterExt<T: DdsType + FlatStruct + Send + Sync + 'static> {
    writer: Arc<DataWriter<T>>,
    backend: Arc<dyn SlotBackend>,
    active_readers_mask: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType + FlatStruct + Send + Sync + 'static> FlatWriterExt<T> {
    /// Wrappt einen DataWriter + Slot-Backend.
    #[must_use]
    pub fn new(
        writer: Arc<DataWriter<T>>,
        backend: Arc<dyn SlotBackend>,
        active_readers_mask: u32,
    ) -> Self {
        Self {
            writer,
            backend,
            active_readers_mask,
            _t: PhantomData,
        }
    }

    /// Spec §8.1 `write_flat` — Same-Host-Pfad. UDP-Cross-Host-Pfad
    /// laeuft parallel ueber `writer.write(&sample)`, sodass beide
    /// Reader-Klassen das Sample sehen.
    ///
    /// # Errors
    /// `OutOfResources` bei vollem Slot-Pool, `WireError` bei
    /// CDR-Encode-Failure (Cross-Host-Pfad).
    pub fn write_flat(&self, sample: &T) -> Result<()> {
        // Same-Host (SHM): write ohne CDR-encode.
        let bytes = sample.as_bytes();
        let handle = self
            .backend
            .reserve_slot(self.active_readers_mask)
            .map_err(slot_to_dds)?;
        if let Err(e) = self.backend.commit_slot(handle, bytes) {
            let _ = self.backend.discard_slot(handle);
            return Err(slot_to_dds(e));
        }
        // Cross-Host (UDP): klassischer DCPS-Pfad parallel.
        self.writer.write(sample)
    }

    /// Liefert Referenz auf den DataWriter (sync-API).
    #[must_use]
    pub fn writer(&self) -> &DataWriter<T> {
        &self.writer
    }
}

/// Builder-API fuer Flatdata-Lesepfade. Alternative zur
/// [`DataReader::set_flat_backend`]-Methode.
pub struct FlatReaderExt<T: DdsType + FlatStruct + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    backend: Arc<dyn SlotBackend>,
    reader_index: u8,
    last_sn: core::sync::atomic::AtomicU32,
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType + FlatStruct + Send + Sync + 'static> FlatReaderExt<T> {
    /// Wrappt einen DataReader + Slot-Backend.
    #[must_use]
    pub fn new(
        reader: Arc<DataReader<T>>,
        backend: Arc<dyn SlotBackend>,
        reader_index: u8,
    ) -> Self {
        Self {
            reader,
            backend,
            reader_index,
            last_sn: core::sync::atomic::AtomicU32::new(u32::MAX),
            _t: PhantomData,
        }
    }

    /// Spec §9.1 `read_flat` — bevorzugt Same-Host-SHM, falls
    /// Slot vorhanden; Type-Hash-Validation gegen Backend-Hash.
    ///
    /// # Errors
    /// `PreconditionNotMet` bei Type-Hash-Mismatch oder Slot-Lock-
    /// Poisoning.
    pub fn read_flat(&self) -> Result<Option<T>> {
        // Spec §6.1: Type-Hash Cross-Validation.
        if let Some(backend_hash) = self.backend.type_hash() {
            if backend_hash != T::TYPE_HASH {
                return Err(DdsError::PreconditionNotMet {
                    reason: "flatdata: TYPE_HASH mismatch (schema drift)",
                });
            }
        }
        scan_slots::<T>(self.backend.as_ref(), self.reader_index, &self.last_sn)
    }

    /// Liefert Referenz auf den DataReader (sync-API).
    #[must_use]
    pub fn reader(&self) -> &DataReader<T> {
        &self.reader
    }
}
