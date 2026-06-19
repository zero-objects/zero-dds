// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// XCDR2 Endian-Mode-Enum.
// Spec: zerodds-xcdr2-csharp-1.0 §2 + zerodds-xcdr2-bindings-conformance-1.0 §3.

namespace ZeroDDS.Cdr;

/// <summary>
/// Endianness selection for the XCDR2 encoder/decoder.
/// PLAIN_CDR2 LE is the default wire format per Conformance spec §3
/// (`0x00 0x01 0x00 0x00`-Encoding-Header).
/// </summary>
public enum EndianMode
{
    /// <summary>Little-endian (default for PLAIN_CDR2).</summary>
    LittleEndian,

    /// <summary>Big-endian (for the key hash `PlainCdr2BeKeyHolder`, XTypes §7.6.8).</summary>
    BigEndian,
}
