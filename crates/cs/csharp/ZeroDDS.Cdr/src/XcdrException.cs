// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runtime error for the XCDR2 encoder/decoder.

using System;

namespace ZeroDDS.Cdr;

/// <summary>
/// Thrown by the XCDR2 encoder/decoder on wire-format errors
/// (z.B. Bounds-Verletzung, ungueltiger DHEADER, unbekanntes LC).
/// </summary>
public sealed class XcdrException : Exception
{
    /// <summary>Default-Constructor.</summary>
    public XcdrException() : base() { }

    /// <summary>Constructor with an error message.</summary>
    public XcdrException(string message) : base(message) { }

    /// <summary>Constructor with an error message + inner exception.</summary>
    public XcdrException(string message, Exception inner) : base(message, inner) { }
}
