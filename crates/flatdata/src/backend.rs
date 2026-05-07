// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `SlotBackend`-Trait — abstrahiert das Slot-Storage-Backend.
//!
//! ADR-0003: drei produktive Backends — in-memory (Default fuer
//! Tests + Single-Process), POSIX shm/mmap (Cross-Process Same-Host
//! per `posix-mmap`-Feature), Iceoryx2 (optionale Bridge per
//! `iceoryx2-bridge`-Feature). Alle drei implementieren denselben
//! Trait, sodass `FlatWriter`/`FlatReader` generisch ueber dem
//! Backend sind.

use alloc::vec::Vec;

use crate::allocator::{SlotError, SlotHandle};
use crate::slot::{ReaderMask, SlotHeader};

/// Backend-Trait fuer SHM-Slot-Allocator.
///
/// Implementer:
/// - [`crate::InMemorySlotAllocator`] — In-Memory-Backend, Default
///   fuer Single-Process-Tests und als Referenz-Implementation.
/// - `PosixSlotAllocator` — POSIX-`shm_open` + `mmap`,
///   Cross-Process-Same-Host (Feature `posix-mmap`).
/// - `Iceoryx2SlotAdapter` — Iceoryx2-Bridge (optional Feature
///   `iceoryx2-bridge`).
pub trait SlotBackend: Send + Sync {
    /// Reserviert einen freien Slot. Caller schreibt danach via
    /// `commit_slot` und veroeffentlicht damit das Sample.
    ///
    /// # Errors
    /// `NoFreeSlot` wenn alle Slots geloant oder unfertig.
    fn reserve_slot(&self, active_readers_mask: ReaderMask) -> Result<SlotHandle, SlotError>;

    /// Schreibt sample-bytes in den Slot und setzt SlotHeader
    /// `{ sn, sample_size, reader_mask=0 }`. Liefert die SN.
    ///
    /// # Errors
    /// `OutOfBounds`, `SampleTooLarge`, oder Lock-Poison.
    fn commit_slot(&self, handle: SlotHandle, bytes: &[u8]) -> Result<u32, SlotError>;

    /// Verwirft einen Loan ohne Commit. Slot wird wieder frei.
    ///
    /// # Errors
    /// `OutOfBounds` oder Lock-Poison.
    fn discard_slot(&self, handle: SlotHandle) -> Result<(), SlotError>;

    /// Liest Slot-Header + Daten-Bytes (kopiert).
    ///
    /// # Errors
    /// `OutOfBounds` oder Lock-Poison.
    fn read_slot(&self, handle: SlotHandle) -> Result<(SlotHeader, Vec<u8>), SlotError>;

    /// Setzt das Bit `reader_index` im `reader_mask` des Slots
    /// (Reader hat gelesen).
    ///
    /// # Errors
    /// `OutOfBounds` oder Lock-Poison.
    fn mark_read(&self, handle: SlotHandle, reader_index: u8) -> Result<(), SlotError>;

    /// Setzt das `reader_index`-Bit retroaktiv auf allen Slots
    /// (SPDP-Lease-Expiry).
    ///
    /// # Errors
    /// Lock-Poison.
    fn mark_reader_disconnected(&self, reader_index: u8) -> Result<(), SlotError>;

    /// Anzahl konfigurierter Slots.
    ///
    /// # Errors
    /// Lock-Poison.
    fn slot_count(&self) -> Result<usize, SlotError>;

    /// Gesamt-Slot-Groesse (Header + Daten + Padding); fuer Discovery.
    fn slot_total_size(&self) -> usize;

    /// Daten-Bereich pro Slot (ohne Header, ohne Padding).
    fn slot_capacity(&self) -> usize;

    /// Spec §6.1: TYPE_HASH des Topic-Type, falls dem Backend
    /// bekannt. `None` = Backend trackt keinen Hash; Caller muss
    /// auf andere Weise (z.B. Discovery) verifizieren. Default: None.
    fn type_hash(&self) -> Option<[u8; 16]> {
        None
    }
}
