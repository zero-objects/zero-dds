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
    /// The serialized COMPLETE <c>TypeObject</c> (XTypes 1.3 §7.3.4) of this
    /// type — the exact XCDR-LE bytes `idlc csharp` emits from the shared
    /// `zerodds_idl::semantics` source (F-TYPES-3 / #24). The DDS layer hands
    /// these to <c>zerodds_pub_create_datawriter_typed</c> /
    /// <c>zerodds_sub_create_datareader_typed</c>, which register the object and
    /// derive + advertise the strongly-hashed <c>TypeIdentifier</c> — identical
    /// across all bindings for the same IDL. Defaults to empty; a generated
    /// TypeSupport for a type without a lowerable TypeObject (union / fixed /
    /// any member) leaves it empty and the runtime falls back to the
    /// byte-oriented create.
    /// </summary>
    byte[] TypeObject => System.Array.Empty<byte>();

    /// <summary>
    /// Encodes `sample` as an XCDR2 little-endian byte array (default encoding).
    /// </summary>
    byte[] Encode(T sample);

    /// <summary>
    /// Encodes `sample` with an explicit endianness mode.
    /// </summary>
    byte[] Encode(T sample, EndianMode endian);

    /// <summary>
    /// Encodes `sample` in the given byte order and XCDR representation
    /// (<paramref name="representation"/>: 1 = XCDR2, 0 = XCDR1 / classic CDR).
    /// The default ignores the representation; generated TypeSupports honor it.
    /// </summary>
    byte[] Encode(T sample, EndianMode endian, int representation) => Encode(sample, endian);

    /// <summary>
    /// Decodes `bytes` and returns a newly constructed sample.
    /// </summary>
    /// <exception cref="XcdrException">On wire-format errors.</exception>
    T Decode(ReadOnlySpan<byte> bytes);

    /// <summary>
    /// Decodes `bytes` interpreted in the given byte order (symmetric with
    /// <see cref="Encode(T, EndianMode)"/>). The DCPS layer derives the order
    /// from the encapsulation header; the parameterless overload assumes
    /// little-endian (the canonical XCDR2 wire). The default delegates to the
    /// little-endian <see cref="Decode(ReadOnlySpan{byte})"/> — generated
    /// TypeSupports override it to honor the byte order.
    /// </summary>
    /// <exception cref="XcdrException">On wire-format errors.</exception>
    T Decode(ReadOnlySpan<byte> bytes, EndianMode endian) => Decode(bytes);

    /// <summary>
    /// Decodes `bytes` in the given byte order and XCDR representation
    /// (<paramref name="representation"/>: 1 = XCDR2, 0 = XCDR1 / classic CDR).
    /// The DCPS layer derives both from the encapsulation header. The default
    /// ignores the representation (assumes XCDR2) — generated TypeSupports
    /// override it to read the classic-CDR / PL_CDR1 wire.
    /// </summary>
    /// <exception cref="XcdrException">On wire-format errors.</exception>
    T Decode(ReadOnlySpan<byte> bytes, EndianMode endian, int representation) => Decode(bytes, endian);

    /// <summary>
    /// Computes the 16-byte key hash per XTypes §7.6.8 (MD5 over the
    /// `PlainCdr2BeKeyHolder` of the `@key` fields).
    /// Returns 16 zero bytes when `IsKeyed == false`.
    /// </summary>
    byte[] KeyHash(T sample);
}
