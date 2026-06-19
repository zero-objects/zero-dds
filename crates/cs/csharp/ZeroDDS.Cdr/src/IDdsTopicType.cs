// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// TypeSupport interface for XCDR2-encodable DDS topic types.
// Spec: zerodds-xcdr2-csharp-1.0 §2 / §3.

using System;

namespace ZeroDDS.Cdr;

/// <summary>
/// Generic TypeSupport interface per zerodds-xcdr2-csharp-1.0 §2.
/// For each IDL `struct`, `idl-csharp` emits a `*TypeSupport` singleton
/// that implements this interface.
/// </summary>
/// <typeparam name="T">Concrete DDS sample type (reference or value type, MUST be non-null).</typeparam>
public interface IDdsTopicType<T> where T : notnull
{
    /// <summary>
    /// DDS type name per the spec convention `Module1::Module2::Struct` (ASCII, max 256 bytes).
    /// Ends up in PID_TYPE_NAME (discovery) and TypeIdentifier lookup.
    /// </summary>
    string TypeName { get; }

    /// <summary>`true` if at least one member carries `@key`.</summary>
    bool IsKeyed { get; }

    /// <summary>Type extensibility per XTypes §7.2.2.4.4.</summary>
    ExtensibilityKind Extensibility { get; }

    /// <summary>
    /// Encodes `sample` as an XCDR2 little-endian byte array (default encoding).
    /// </summary>
    byte[] Encode(T sample);

    /// <summary>
    /// Encodes `sample` with an explicit endianness mode.
    /// </summary>
    byte[] Encode(T sample, EndianMode endian);

    /// <summary>
    /// Decodes `bytes` and returns a newly constructed sample.
    /// </summary>
    /// <exception cref="XcdrException">On wire-format errors.</exception>
    T Decode(ReadOnlySpan<byte> bytes);

    /// <summary>
    /// Computes the 16-byte key hash per XTypes §7.6.8 (MD5 over the
    /// `PlainCdr2BeKeyHolder` of the `@key` fields).
    /// Returns 16 zero bytes when `IsKeyed == false`.
    /// </summary>
    byte[] KeyHash(T sample);
}
