// SPDX-License-Identifier: Apache-2.0
// DDS-TS 1.0 — Runtime descriptor types.
//
// This file is part of the @zerodds/types runtime package.
// It defines the type-level surface required by the
// Descriptor-Runtime Profile of DDS-TS 1.0 (Chapter 8 of the
// vendor specification).

/** XTypes 1.3 ExtensibilityKind. */
export type ExtensibilityKind = "final" | "appendable" | "mutable";

/** Primitive type names referenced by DdsTypeRef. */
export type PrimitiveName =
  | "boolean"
  | "octet"
  | "int16"
  | "uint16"
  | "int32"
  | "uint32"
  | "int64"
  | "uint64"
  | "float"
  | "double"
  | "longDouble"
  | "char"
  | "wchar";

/**
 * Side-table type reference.
 *
 * The `ref` kind carries a qualified IDL name; consumers resolve
 * it via `lookupType` from the registry. This indirection enables
 * mutually-recursive struct/union references and keeps the
 * descriptor graph JSON-roundtrippable.
 */
export type DdsTypeRef =
  | { kind: "primitive"; name: PrimitiveName }
  | { kind: "bitfield";  width: number }
  | { kind: "string";    bound?: number; wide: boolean }
  | { kind: "sequence";  element: DdsTypeRef; bound?: number }
  | { kind: "array";     element: DdsTypeRef; lengths: ReadonlyArray<number> }
  | { kind: "map";       key: DdsTypeRef; value: DdsTypeRef; bound?: number }
  | { kind: "ref";       name: string }
  | { kind: "any" };

/** DescriptorKind discriminator on DdsTypeDescriptor. */
export type DescriptorKind =
  | "struct"
  | "union"
  | "enum"
  | "bitset"
  | "bitmask"
  | "exception"
  | "alias"
  | "interface";

/** Per-member metadata within a DdsTypeDescriptor. */
export interface DdsMemberDescriptor {
  readonly name:           string;
  readonly id:             number;
  readonly type:           DdsTypeRef;
  readonly key:            boolean;
  readonly optional:       boolean;
  readonly mustUnderstand: boolean;
  readonly default?:       unknown;
  readonly unit?:          string;
  readonly min?:           unknown;
  readonly max?:           unknown;
  readonly hashid?:        string;
  /** Union case-member only. */
  readonly labels?:            ReadonlyArray<number | string | boolean>;
  /** Map key with non-value-equality semantics. */
  readonly keyEqualityHazard?: boolean;
}

/** First-class metadata for an IDL type. */
export interface DdsTypeDescriptor<T = unknown> {
  readonly kind:                DescriptorKind;
  readonly name:                string;
  readonly extensibility:       ExtensibilityKind;
  readonly nested:              boolean;
  readonly topic?:              string;
  readonly autoid?:             "sequential" | "hash";
  readonly bitBound?:           number;
  /** Union descriptors only. */
  readonly hasDefault?:         boolean;
  /** Union descriptors only. */
  readonly hasImplicitDefault?: boolean;
  readonly fields:              ReadonlyArray<DdsMemberDescriptor>;
  readonly typeGuard:           (v: unknown) => v is T;
}

/** Helper extracting T from DdsTypeDescriptor<T>. */
export type GuardedTypeOf<D> =
  D extends DdsTypeDescriptor<infer T> ? T : never;
