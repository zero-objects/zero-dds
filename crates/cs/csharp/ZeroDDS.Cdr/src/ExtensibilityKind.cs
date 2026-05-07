// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// XCDR2 Extensibility-Kind-Enum.
// Spec: XTypes 1.3 §7.2.2.4.4 + zerodds-xcdr2-csharp-1.0 §6.

namespace ZeroDDS.Cdr;

/// <summary>
/// Type-Extensibility laut OMG XTypes 1.3 §7.2.2.4.4.
/// Steuert das Wire-Layout: Final = PLAIN_CDR2 ohne Header,
/// Appendable = DELIMITED_CDR2 mit DHEADER, Mutable = PL_CDR2 mit EMHEADER.
/// </summary>
public enum ExtensibilityKind
{
    /// <summary>`@final` - kein DHEADER, keine Evolution erlaubt.</summary>
    Final,

    /// <summary>`@appendable` (Default) - 4-Byte uint32 DHEADER prefixed, append-only Evolution.</summary>
    Appendable,

    /// <summary>`@mutable` - PL_CDR2 mit EMHEADER pro Member, freie Evolution via @id.</summary>
    Mutable,
}
