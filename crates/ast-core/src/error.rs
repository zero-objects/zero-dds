// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Error type for the AST core infrastructure. Spec §10.

use core::fmt;

/// Failure of a core infrastructure operation.
///
/// The public API returns these; it never panics. Spec §10.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AstCoreError {
    /// A handle was used after its node was removed. The slot may since
    /// have been reused by a different node; reading through the stale
    /// handle would have addressed that unrelated node.
    StaleHandle {
        /// Arena slot the handle referred to.
        slot: u32,
        /// Generation the handle carried.
        expected: u32,
        /// Generation the slot carries now.
        found: u32,
    },
    /// A handle referred to a slot the arena never allocated.
    UnknownSlot {
        /// Slot index out of range.
        slot: u32,
    },
    /// The arena exhausted its slot or generation space, or an identity
    /// allocator exhausted its counter.
    Exhausted,
    /// A snapshot could not be decoded. Spec §9.
    MalformedSnapshot {
        /// Byte offset at which decoding failed.
        offset: usize,
        /// What the decoder expected there.
        expected: &'static str,
    },
    /// A commit was rejected by validation and rolled back. Spec §5.1.
    CommitRejected {
        /// Why the commit was rejected.
        reason: &'static str,
    },
}

impl fmt::Display for AstCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleHandle {
                slot,
                expected,
                found,
            } => write!(
                f,
                "stale handle: slot {slot} carried generation {expected}, arena is at {found}"
            ),
            Self::UnknownSlot { slot } => write!(f, "unknown arena slot {slot}"),
            Self::Exhausted => write!(f, "arena or identity space exhausted"),
            Self::MalformedSnapshot { offset, expected } => {
                write!(
                    f,
                    "malformed snapshot at byte {offset}: expected {expected}"
                )
            }
            Self::CommitRejected { reason } => write!(f, "commit rejected: {reason}"),
        }
    }
}

impl core::error::Error for AstCoreError {}
