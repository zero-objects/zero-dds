// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// XCDR2 Endian-Mode-Enum.
// Spec: zerodds-xcdr2-csharp-1.0 §2 + zerodds-xcdr2-bindings-conformance-1.0 §3.

namespace ZeroDDS.Cdr;

/// <summary>
/// Endianness-Auswahl fuer den XCDR2-Encoder/Decoder.
/// PLAIN_CDR2 LE ist Default-Wire-Format laut Conformance-Spec §3
/// (`0x00 0x01 0x00 0x00`-Encoding-Header).
/// </summary>
public enum EndianMode
{
    /// <summary>Little-Endian (Default fuer PLAIN_CDR2).</summary>
    LittleEndian,

    /// <summary>Big-Endian (fuer Key-Hash `PlainCdr2BeKeyHolder`, XTypes §7.6.8).</summary>
    BigEndian,
}
