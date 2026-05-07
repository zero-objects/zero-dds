// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// types.ts — Public-facing TypeSupport-Interface fuer XCDR2-Codegen.
// Konformanz: zerodds-xcdr2-ts-1.0 §2 + §3.

/// Extensibility-Kind je IDL-Annotation.
/// `final`      -> kein DHEADER (PLAIN_CDR2).
/// `appendable` -> 4-Byte-uint32-DHEADER (DELIMITED_CDR2).
/// `mutable`    -> EMHEADER pro Member (PL_CDR2).
export type ExtensibilityKind = 'final' | 'appendable' | 'mutable';

/// Endian-Mode fuer Wire-Bytes.
/// Default in zerodds-xcdr2-bindings-conformance-1.0 §3 ist `le`.
export type EndianMode = 'le' | 'be';

/// Generisches TypeSupport-Interface; pro IDL-Struct emittiert
/// `idl-ts` ein `*TypeSupport`-Const, das dieses Interface erfuellt.
export interface DdsTopicType<T> {
    /// DDS-Type-Name nach §5 Konvention `Module::Sub::Struct`.
    readonly typeName: string;
    /// `true` wenn mindestens ein Member `@key` traegt.
    readonly isKeyed: boolean;
    /// XTypes-1.3-Extensibility (§7.2.2.4.4).
    readonly extensibility: ExtensibilityKind;

    /// XCDR2-Encode des Samples ohne RTPS-Encapsulation-Header.
    encode(sample: T, endian?: EndianMode): Uint8Array;

    /// XCDR2-Decode; `length` defaultet auf `bytes.length - offset`.
    decode(bytes: Uint8Array, offset?: number, length?: number): T;

    /// 16-Byte MD5-Key-Hash (`PlainCdr2BeKeyHolder`, BE) oder Null-
    /// Hash falls Type nicht keyed (XTypes §7.6.8).
    keyHash(sample: T): Uint8Array;
}
