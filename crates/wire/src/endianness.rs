// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Byte order.

/// Which byte order a stream is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Endianness {
    /// Most significant byte first. The order a KeyHash holder is always
    /// in, whatever the sample is. DDSI-RTPS 2.5 §9.6.3.8.
    Big,
    /// Least significant byte first.
    Little,
}

impl Endianness {
    /// Both orders.
    pub const ALL: [Self; 2] = [Self::Big, Self::Little];

    /// Whether this is little-endian.
    #[must_use]
    pub const fn is_little(self) -> bool {
        matches!(self, Self::Little)
    }
}
