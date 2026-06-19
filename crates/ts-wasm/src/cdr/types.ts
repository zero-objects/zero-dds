// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// types.ts — public-facing TypeSupport interface for XCDR2 codegen.
// Konformanz: zerodds-xcdr2-ts-1.0 §2 + §3.

/// Extensibility-Kind je IDL-Annotation.
/// `final`      -> no DHEADER (PLAIN_CDR2).
/// `appendable` -> 4-Byte-uint32-DHEADER (DELIMITED_CDR2).
/// `mutable`    -> EMHEADER pro Member (PL_CDR2).
export type ExtensibilityKind = 'final' | 'appendable' | 'mutable';

/// Endian mode for wire bytes.
/// Default in zerodds-xcdr2-bindings-conformance-1.0 §3 ist `le`.
export type EndianMode = 'le' | 'be';

/// Generisches TypeSupport-Interface; pro IDL-Struct emittiert
/// `idl-ts` ein `*TypeSupport`-Const, das dieses Interface erfuellt.
export interface DdsTopicType<T> {
    /// DDS type name per the §5 convention `Module::Sub::Struct`.
    readonly typeName: string;
    /// `true` if at least one member carries `@key`.
    readonly isKeyed: boolean;
    /// XTypes-1.3-Extensibility (§7.2.2.4.4).
    readonly extensibility: ExtensibilityKind;

    /// XCDR2 encode of the sample without an RTPS encapsulation header.
    encode(sample: T, endian?: EndianMode): Uint8Array;

    /// XCDR2-Decode; `length` defaultet auf `bytes.length - offset`.
    decode(bytes: Uint8Array, offset?: number, length?: number): T;

    /// 16-byte MD5 key hash (`PlainCdr2BeKeyHolder`, BE) or null-
    /// Hash if the type is not keyed (XTypes §7.6.8).
    keyHash(sample: T): Uint8Array;
}
