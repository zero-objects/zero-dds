// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// TypeSupport-Interface fuer XCDR2-codierbare DDS-Topic-Types.
// Spec: zerodds-xcdr2-csharp-1.0 §2 / §3.

using System;

namespace ZeroDDS.Cdr;

/// <summary>
/// Generisches TypeSupport-Interface laut zerodds-xcdr2-csharp-1.0 §2.
/// Pro IDL-`struct` emittiert `idl-csharp` einen `*TypeSupport`-Singleton
/// der dieses Interface implementiert.
/// </summary>
/// <typeparam name="T">Konkreter DDS-Sample-Type (Reference- oder Value-Type, MUSS not-null sein).</typeparam>
public interface IDdsTopicType<T> where T : notnull
{
    /// <summary>
    /// DDS-Type-Name laut Spec-Konvention `Module1::Module2::Struct` (ASCII, max 256 Bytes).
    /// Landet in PID_TYPE_NAME (Discovery) und TypeIdentifier-Lookup.
    /// </summary>
    string TypeName { get; }

    /// <summary>`true` falls mindestens ein Member `@key` traegt.</summary>
    bool IsKeyed { get; }

    /// <summary>Type-Extensibility laut XTypes §7.2.2.4.4.</summary>
    ExtensibilityKind Extensibility { get; }

    /// <summary>
    /// Encodes `sample` als XCDR2 Little-Endian Byte-Array (Default-Encoding).
    /// </summary>
    byte[] Encode(T sample);

    /// <summary>
    /// Encodes `sample` mit explizitem Endianness-Mode.
    /// </summary>
    byte[] Encode(T sample, EndianMode endian);

    /// <summary>
    /// Decodes `bytes` und liefert ein neu konstruiertes Sample.
    /// </summary>
    /// <exception cref="XcdrException">Bei Wire-Format-Fehlern.</exception>
    T Decode(ReadOnlySpan<byte> bytes);

    /// <summary>
    /// Berechnet den 16-Byte Key-Hash gemaess XTypes §7.6.8 (MD5 ueber
    /// `PlainCdr2BeKeyHolder` der `@key`-Felder).
    /// Liefert 16 Null-Bytes wenn `IsKeyed == false`.
    /// </summary>
    byte[] KeyHash(T sample);
}
