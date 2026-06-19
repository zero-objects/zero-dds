// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// types.ts — public-facing TypeSupport interface for the XCDR2 codegen.
// Conformance: zerodds-xcdr2-ts-1.0 §2 + §3.

import type { Xcdr2Reader } from './reader.js';
import type { Xcdr2Writer } from './writer.js';

/// Extensibility kind per IDL annotation.
/// `final`      -> no DHEADER (PLAIN_CDR2).
/// `appendable` -> 4-byte uint32 DHEADER (DELIMITED_CDR2).
/// `mutable`    -> EMHEADER per member (PL_CDR2).
export type ExtensibilityKind = 'final' | 'appendable' | 'mutable';

/// Endian mode for wire bytes.
/// The default in zerodds-xcdr2-bindings-conformance-1.0 §3 is `le`.
export type EndianMode = 'le' | 'be';

/// Generic TypeSupport interface; per IDL struct,
/// `idl-ts` emits a `*TypeSupport` const that fulfills this interface.
export interface DdsTopicType<T> {
    /// DDS type name per the §5 convention `Module::Sub::Struct`.
    readonly typeName: string;
    /// `true` if at least one member carries `@key`.
    readonly isKeyed: boolean;
    /// XTypes 1.3 extensibility (§7.2.2.4.4).
    readonly extensibility: ExtensibilityKind;

    /// XCDR2 encode of the sample without the RTPS encapsulation header.
    encode(sample: T, endian?: EndianMode): Uint8Array;

    /// XCDR2 decode; `length` defaults to `bytes.length - offset`.
    decode(bytes: Uint8Array, offset?: number, length?: number): T;

    /// Encodes the sample's body into an existing writer (shared CDR stream).
    /// Used to embed this type as a nested member of another type so that
    /// XCDR2 alignment stays relative to the outer stream. `encode` is
    /// `encodeInto` into a fresh writer.
    encodeInto(w: Xcdr2Writer, sample: T): void;

    /// Decodes the type's body from an existing reader (shared CDR stream) —
    /// the nested-member counterpart of `decode`.
    decodeFrom(r: Xcdr2Reader): T;

    /// 16-byte MD5 key hash (`PlainCdr2BeKeyHolder`, BE) or the null
    /// hash if the type is not keyed (XTypes §7.6.8).
    keyHash(sample: T): Uint8Array;
}
