// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runtime-Fehler fuer XCDR2-Encoder/Decoder.

using System;

namespace ZeroDDS.Cdr;

/// <summary>
/// Wird vom XCDR2-Encoder/Decoder bei Wire-Format-Fehlern geworfen
/// (z.B. Bounds-Verletzung, ungueltiger DHEADER, unbekanntes LC).
/// </summary>
public sealed class XcdrException : Exception
{
    /// <summary>Default-Constructor.</summary>
    public XcdrException() : base() { }

    /// <summary>Constructor mit Fehlermeldung.</summary>
    public XcdrException(string message) : base(message) { }

    /// <summary>Constructor mit Fehlermeldung + Inner-Exception.</summary>
    public XcdrException(string message, Exception inner) : base(message, inner) { }
}
