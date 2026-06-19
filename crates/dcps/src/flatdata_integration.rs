// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Flatdata integration (ADR-0005, opt-in via the
//! `flatdata-integration` feature).
//!
//! Provides `DataWriter::write_flat` and `DataReader::read_flat` as
//! conditional methods with a `T: FlatStruct` bound. The default build
//! does not see these methods — the caller must explicitly enable
//! `--features flatdata-integration`.
//!
//! # Architecture
//!
//! Spec `zerodds-flatdata-1.0` §8/§9 requires the spec-conformant
//! methods directly on `DataWriter<T>` / `DataReader<T>`:
//!
//! 1. `DataWriter::set_flat_backend(backend, mask)` and
//!    `DataReader::set_flat_backend(backend, reader_index)` couple the
//!    slot backend to the entity. Configured once after a discovery
//!    match.
//! 2. `DataWriter::write_flat(sample)` writes into the SHM slot
//!    (Spec §4.1) **and** sends UDP-DATA in parallel for cross-host
//!    readers (§4.2 cross-host fallback). Without a configured backend
//!    the method falls back to the classic UDP path.
//! 3. `DataReader::read_flat()` returns the latest sample from the
//!    slot pool (Spec §9.1) and falls back to `take()` if no slot is
//!    present.
//!
//! In addition there is the builder variant `FlatWriterExt` /
//! `FlatReaderExt` (see further down in this module) as an alternative
//! API that does not require setting state on the entity object —
//! useful when a caller wants to use multiple slot backends in
//! parallel with the same DataWriter.
//!
//! # Safety rationale
//!
//! `from_bytes_unchecked` is an `unsafe fn`. The caller here has
//! verified the type hash + sample size (see `read_flat`). Local
//! `#[allow(unsafe_code)]` with a per-block SAFETY comment.
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

/// Converts `SlotError` into `DdsError`.
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
        SlotError::InPlaceUnsupported => DdsError::Unsupported {
            feature: "flatdata: in-place loan (slot_data_ptr/commit_in_place)",
        },
    }
}

// ============================================================================
// Spec-conformant methods directly on the DataWriter/DataReader (§8.1, §9.1)
// ============================================================================

impl<T: DdsType + FlatStruct + Send + Sync + 'static> DataWriter<T> {
    /// Sets the flatdata SlotBackend for the same-host zero-copy path.
    /// With `Some(backend)`, [`Self::write_flat`] writes the sample
    /// directly into an SHM slot (Spec §4.1) and sends UDP-DATA in
    /// parallel for cross-host readers. `active_readers_mask` lists the
    /// reader bits relevant for slot reuse (Spec §5.1 slot refcount).
    ///
    /// `None` disables the SHM path — subsequent `write_flat()` calls
    /// fall back to the classic UDP path.
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

    /// Spec §8.1 `write_flat` — writes a `FlatStruct` sample directly
    /// into the SHM slot (no CDR encode) and sends UDP-DATA in parallel
    /// for cross-host readers (Spec §4.2). Without a configured backend
    /// (see [`Self::set_flat_backend`]) the call falls back to the
    /// classic UDP path.
    ///
    /// # Errors
    /// - `OutOfResources` on a full slot pool or `SampleTooLarge`.
    /// - `WireError` on a CDR encode failure of the UDP cross-host path.
    /// - `PreconditionNotMet` on slot-lock poisoning.
    pub fn write_flat(&self, sample: &T) -> Result<()> {
        let backend_snapshot = self
            .flat_backend
            .lock()
            .map_err(|_| DdsError::PreconditionNotMet {
                reason: "flatdata: backend mutex poisoned",
            })?
            .clone();

        if let Some((backend, mask)) = backend_snapshot {
            // Same host (SHM): write without CDR encode.
            let bytes = sample.as_bytes();
            let handle = backend.reserve_slot(mask).map_err(slot_to_dds)?;
            if let Err(e) = backend.commit_slot(handle, bytes) {
                let _ = backend.discard_slot(handle);
                return Err(slot_to_dds(e));
            }
            // Cross host (UDP): classic DCPS path in parallel
            // (Spec §4.2 cross-host fallback).
            self.write(sample)
        } else {
            // No backend configured → pure UDP path.
            self.write(sample)
        }
    }
}

impl<T: DdsType + FlatStruct + Send + Sync + 'static> DataReader<T> {
    /// Sets the flatdata SlotBackend for the same-host zero-copy read
    /// path. `reader_index` is the bit (0..31) in `slot.reader_mask`
    /// that this reader sets after a successful read (Spec §5.1).
    ///
    /// `None` disables the SHM path — subsequent `read_flat()` calls
    /// fall back to `take()`.
    ///
    /// Spec: `zerodds-flatdata-1.0` §4.1 + §9.
    pub fn set_flat_backend(&self, backend: Option<Arc<dyn SlotBackend>>, reader_index: u8) {
        use core::sync::atomic::AtomicU32;
        if let Ok(mut slot) = self.flat_backend.lock() {
            *slot = backend.map(|b| (b, reader_index, AtomicU32::new(u32::MAX)));
        }
    }

    /// Spec §9.1 `read_flat` — prefers same-host SHM if a backend is
    /// configured; falls back to [`Self::take`] if no slot is present
    /// or no backend is set.
    ///
    /// Spec §6.1: checks `backend.type_hash()` against `T::TYPE_HASH`;
    /// on mismatch returns `Err(PreconditionNotMet)` without
    /// dereferencing the slot (protection against schema drift).
    ///
    /// # Errors
    /// `PreconditionNotMet` on type-hash mismatch or slot-lock
    /// poisoning.
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
        // Spec §6.1: type-hash cross-validation.
        if let Some(backend_hash) = backend.type_hash() {
            if backend_hash != T::TYPE_HASH {
                return Err(DdsError::PreconditionNotMet {
                    reason: "flatdata: TYPE_HASH mismatch (schema drift)",
                });
            }
        }
        scan_slots::<T>(backend.as_ref(), *reader_index, last_sn)
    }

    /// Spec §4.2 event-driven `read_flat`. Like [`Self::read_flat`] but blocks
    /// on the backend's notify (POSIX cross-process futex / in-memory condvar —
    /// NO busy-poll, no UDP roundtrip) until a same-host SHM sample arrives or
    /// `timeout` elapses (`Ok(None)`). Falls back to a single `read_flat` when
    /// the backend has no notify support.
    ///
    /// # Errors
    /// As [`Self::read_flat`].
    pub fn read_flat_blocking(&self, timeout: core::time::Duration) -> Result<Option<T>> {
        let deadline = std::time::Instant::now() + timeout;
        // Clone the backend Arc so the wait does not hold the flat_backend lock.
        let backend = {
            let slot = self
                .flat_backend
                .lock()
                .map_err(|_| DdsError::PreconditionNotMet {
                    reason: "flatdata: backend mutex poisoned",
                })?;
            match slot.as_ref() {
                Some((b, _, _)) => Arc::clone(b),
                None => return Ok(None),
            }
        };
        loop {
            if let Some(sample) = self.read_flat()? {
                return Ok(Some(sample));
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            // Capture gen, re-check (lost-wakeup-free), then park on the notify.
            let g = backend.notify_generation();
            if let Some(sample) = self.read_flat()? {
                return Ok(Some(sample));
            }
            backend.wait_for_change(g, deadline - now);
        }
    }
}

/// Scan helper: returns the newest unread sample from the slot pool
/// (Spec §9.1). Shared between `DataReader::read_flat` and
/// [`FlatReaderExt::read_flat`].
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
        // SAFETY: WIRE_SIZE checked; the FlatStruct bound guarantees
        // layout consistency.
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
// Builder API (alternative, for callers that want to use multiple
// backends in parallel with the same entity)
// ============================================================================

/// Builder API for flatdata write paths. Alternative to the
/// [`DataWriter::set_flat_backend`] method: encapsulates a slot backend
/// and exposes `write_flat` without state on the entity.
pub struct FlatWriterExt<T: DdsType + FlatStruct + Send + Sync + 'static> {
    writer: Arc<DataWriter<T>>,
    backend: Arc<dyn SlotBackend>,
    active_readers_mask: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType + FlatStruct + Send + Sync + 'static> FlatWriterExt<T> {
    /// Wraps a DataWriter + slot backend.
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

    /// Spec §8.1 `write_flat` — same-host path. The UDP cross-host path
    /// runs in parallel via `writer.write(&sample)`, so that both
    /// reader classes see the sample.
    ///
    /// # Errors
    /// `OutOfResources` on a full slot pool, `WireError` on a CDR
    /// encode failure (cross-host path).
    pub fn write_flat(&self, sample: &T) -> Result<()> {
        // Same host (SHM): write without CDR encode.
        let bytes = sample.as_bytes();
        let handle = self
            .backend
            .reserve_slot(self.active_readers_mask)
            .map_err(slot_to_dds)?;
        if let Err(e) = self.backend.commit_slot(handle, bytes) {
            let _ = self.backend.discard_slot(handle);
            return Err(slot_to_dds(e));
        }
        // Cross host (UDP): classic DCPS path in parallel.
        self.writer.write(sample)
    }

    /// Returns a reference to the DataWriter (sync API).
    #[must_use]
    pub fn writer(&self) -> &DataWriter<T> {
        &self.writer
    }
}

/// Builder API for flatdata read paths. Alternative to the
/// [`DataReader::set_flat_backend`] method.
pub struct FlatReaderExt<T: DdsType + FlatStruct + Send + Sync + 'static> {
    reader: Arc<DataReader<T>>,
    backend: Arc<dyn SlotBackend>,
    reader_index: u8,
    last_sn: core::sync::atomic::AtomicU32,
    _t: PhantomData<fn() -> T>,
}

impl<T: DdsType + FlatStruct + Send + Sync + 'static> FlatReaderExt<T> {
    /// Wraps a DataReader + slot backend.
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

    /// Spec §9.1 `read_flat` — prefers same-host SHM if a slot is
    /// present; type-hash validation against the backend hash.
    ///
    /// # Errors
    /// `PreconditionNotMet` on type-hash mismatch or slot-lock
    /// poisoning.
    pub fn read_flat(&self) -> Result<Option<T>> {
        // Spec §6.1: type-hash cross-validation.
        if let Some(backend_hash) = self.backend.type_hash() {
            if backend_hash != T::TYPE_HASH {
                return Err(DdsError::PreconditionNotMet {
                    reason: "flatdata: TYPE_HASH mismatch (schema drift)",
                });
            }
        }
        scan_slots::<T>(self.backend.as_ref(), self.reader_index, &self.last_sn)
    }

    /// Returns a reference to the DataReader (sync API).
    #[must_use]
    pub fn reader(&self) -> &DataReader<T> {
        &self.reader
    }
}
