// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// XCDR2 extensibility-kind enum.
// Spec: XTypes 1.3 §7.2.2.4.4 + zerodds-xcdr2-csharp-1.0 §6.

namespace ZeroDDS.Cdr;

/// <summary>
/// Type extensibility per OMG XTypes 1.3 §7.2.2.4.4.
/// Controls the wire layout: Final = PLAIN_CDR2 without header,
/// Appendable = DELIMITED_CDR2 with DHEADER, Mutable = PL_CDR2 with EMHEADER.
/// </summary>
public enum ExtensibilityKind
{
    /// <summary>`@final` - no DHEADER, no evolution allowed.</summary>
    Final,

    /// <summary>`@appendable` (default) - 4-byte uint32 DHEADER prefixed, append-only evolution.</summary>
    Appendable,

    /// <summary>`@mutable` - PL_CDR2 with EMHEADER per member, free evolution via @id.</summary>
    Mutable,
}
