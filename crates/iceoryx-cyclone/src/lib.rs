// SPDX-License-Identifier: Apache-2.0
//! Crate `zerodds-iceoryx-cyclone`. Safety classification: **STANDARD**
//! (FFI bridge over `libiceoryx_binding_c`; unsafe is reviewed and bounded).
//!
//! ZeroDDS ↔ Cyclone DDS same-host zero-copy bridge over the **iceoryx C++
//! (POSH)** shared-memory transport — the SHM stack Cyclone uses through its
//! `iox_psmx` plugin (with the `iox-roudi` daemon), as opposed to the
//! iceoryx2-Rust stack [`zerodds_flatdata`] bridges to natively.
//!
//! This crate FFI-binds the stable `libiceoryx_binding_c` C ABI (no C++ name
//! mangling) and speaks Cyclone's PSMX chunk convention directly, so a ZeroDDS
//! process can READ a Cyclone-published sample and WRITE one a Cyclone reader
//! consumes — true cross-vendor same-host zero-copy.
//!
//! Wire facts (reverse-engineered + live-verified, see
//! `docs/interop/cyclone-iceoryx-shm-ground-truth.md`):
//! * iceoryx ServiceDescription `{ "DDS_CYCLONE", <type-name>, ".<topic>" }`;
//! * each chunk's iceoryx user-header is a [`DdsPsmxMetadata`], the user payload
//!   the sample (raw native struct for a self-contained type — `RAW_DATA`).
//!
//! Linux + iceoryx-POSH only: the FFI compiles under
//! `cfg(all(target_os = "linux", feature = "posh"))` and links
//! `libiceoryx_binding_c`. Without the feature the crate exposes only the
//! representation-level [`DdsPsmxMetadata`] type so it builds anywhere.

#![cfg_attr(not(feature = "posh"), allow(dead_code))]

/// Cyclone `dds_psmx_metadata_t` (`dds/ddsc/dds_psmx.h`) — the iceoryx
/// user-header carried alongside every PSMX sample. Field sizes are the
/// live-verified layout: `instance_id` is `u32` (not `u64`), which is what
/// places `sample_size` at the correct offset.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DdsPsmxMetadata {
    /// `dds_loaned_sample_state_t` enum: `2` == `RAW_DATA` (full raw sample).
    pub sample_state: u32,
    /// Reserved, must be `0`.
    pub data_type: u32,
    /// `dds_psmx_instance_id_t` (`u32`).
    pub instance_id: u32,
    /// Byte length of the user payload.
    pub sample_size: u32,
    /// Original writer GUID.
    pub guid: [u8; 16],
    /// Source timestamp (ns).
    pub timestamp: i64,
    /// DDSI status info (write / dispose / unregister).
    pub statusinfo: u32,
    /// RTPS encoding id (`NATIVE` when the payload is unserialized).
    pub cdr_identifier: u16,
    /// CDR options (`0` when unserialized).
    pub cdr_options: u16,
}

impl DdsPsmxMetadata {
    /// `dds_loaned_sample_state_t::DDS_LOANED_SAMPLE_STATE_RAW_DATA`.
    pub const STATE_RAW_DATA: u32 = 2;
    /// `dds_loaned_sample_state_t::DDS_LOANED_SAMPLE_STATE_SERIALIZED_DATA`.
    pub const STATE_SERIALIZED_DATA: u32 = 4;
    /// `cdr_identifier` value for classic CDR little-endian (`CDR_LE = 0x0001`
    /// stored in Cyclone's host-order field). This is what Cyclone stamps on a
    /// serialized iceoryx sample — observed live.
    pub const CDR_LE: u16 = 0x0100;

    /// A metadata header for a **serialized** sample of `size` bytes (classic
    /// CDR / XCDR1 little-endian) from writer `guid` — what Cyclone's `iox_psmx`
    /// writes for a non-self-contained type. Pair with `T::encode_xcdr1`.
    pub fn serialized(size: u32, guid: [u8; 16]) -> Self {
        Self {
            sample_state: Self::STATE_SERIALIZED_DATA,
            data_type: 0,
            instance_id: 0,
            sample_size: size,
            guid,
            timestamp: 0,
            statusinfo: 0,
            cdr_identifier: Self::CDR_LE,
            cdr_options: 0,
        }
    }

    /// `true` if the payload is a serialized (CDR) sample.
    pub fn is_serialized(&self) -> bool {
        self.sample_state == Self::STATE_SERIALIZED_DATA
    }

    /// A metadata header for a raw self-contained sample of `size` bytes from
    /// writer `guid` — what Cyclone's `iox_psmx` writes for a fixed type.
    pub fn raw(size: u32, guid: [u8; 16]) -> Self {
        Self {
            sample_state: Self::STATE_RAW_DATA,
            data_type: 0,
            instance_id: 0,
            sample_size: size,
            guid,
            timestamp: 0,
            statusinfo: 0,
            // 0x00c0 is the RTPS "native sample" marker Cyclone stamps on a
            // raw, unserialized loan (observed in the live capture).
            cdr_identifier: 0x00c0,
            cdr_options: 0,
        }
    }

    /// `true` if the payload is the raw, unserialized sample (native struct).
    pub fn is_raw(&self) -> bool {
        self.sample_state == Self::STATE_RAW_DATA
    }
}

/// The fixed iceoryx service string Cyclone's `iox_psmx` registers under.
pub const CYCLONE_IOX_SERVICE: &str = "DDS_CYCLONE";

/// Builds the iceoryx event string for `topic` in `partition` the way Cyclone
/// does: `"<partition>.<topic>"`, with a leading `"."` for the empty partition.
pub fn iox_event_name(partition: &str, topic: &str) -> String {
    format!("{partition}.{topic}")
}

#[cfg(all(target_os = "linux", feature = "posh"))]
mod posh;
#[cfg(all(target_os = "linux", feature = "posh"))]
pub use posh::{init_runtime, CycloneIoxReader, CycloneIoxWriter};
