// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The descriptor side-table, as the values a TypeScript program can read.
 *
 * DDS-TS 1.0 chapter 8. A TypeScript `interface` is erased at compile
 * time and a plain object literal carries no run-time type identity, so
 * the identity travels beside the data as a first-class value: one
 * {@link DdsTypeDescriptor} per IDL type, emitted by the code generator
 * and registered with the process-wide registry
 * (`registerType` in `./registry.js`).
 *
 * The content set is deliberately close to the `TypeObject` /
 * `MinimalTypeObject` structures of OMG X-Types 1.3 §7.3 — extensibility,
 * member ids, key membership, type references — so that a descriptor can
 * be turned into a `TypeObject` and back without losing anything
 * (DDS-TS 1.0 §6.2).
 *
 * Nothing in this file has a run-time representation: it is types only.
 * The functions that read these shapes live in `./registry.js`,
 * `./dds_any.js` and `./equal.js`.
 */

/**
 * The three extensibility kinds of OMG X-Types 1.3 §7.2.2.4.4.
 *
 * DDS-TS 1.0 §8.1. Spelled in lower case rather than as the
 * `FINAL`/`APPENDABLE`/`MUTABLE` of the IDL annotation, because the
 * descriptor is JSON-roundtrippable and these are its values.
 */
export type ExtensibilityKind = "final" | "appendable" | "mutable";

/**
 * The IDL primitive types, under the names the descriptor uses.
 *
 * DDS-TS 1.0 §8.2. `int16` and not `short`: the IDL 4.2 §7.4.1.4.4.2
 * explicit-width spellings are unambiguous, and a descriptor that said
 * `long` would be read differently by a C consumer than by a Java one.
 * `longDouble` is the sixteen-octet carrier of §8.6, not a `number`.
 *
 * `int8` is one name past the list §8.2 gives. IDL 4.2 §7.4.1.4.4.2
 * declares `int8` alongside `uint8`, the list names `octet` — which is
 * `uint8` — and names nothing for the signed one. A generator meeting
 * `int8 v;` has three choices: write `octet`, which is the wrong
 * signedness; refuse the member, which drops a type IDL admits; or
 * carry a name the list does not have. The third is the only one that
 * does not lose something, and it is additive: every value §8.2 lists
 * still means what §8.2 says.
 */
export type PrimitiveName =
  | "boolean"
  | "octet"
  | "int8"
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
 * The type of one member, in side-table form.
 *
 * DDS-TS 1.0 §8.2. A discriminated union over `kind`, which is what lets
 * a consumer walk a descriptor graph with the compiler checking every
 * arm.
 *
 * The `ref` kind carries the **qualified IDL name** of the referenced
 * type and not a descriptor reference. That rule is normative and it
 * buys three things: mutually-recursive struct and union references that
 * no source-line order could initialise, a value graph with no cycles in
 * it (so `JSON.stringify` round-trips a descriptor), and the same shape
 * OMG X-Types 1.3 uses for `TypeIdentifier` lookup. Resolution happens at
 * use time, through `lookupType(name)`.
 *
 * The `any` kind carries no name at all: the run-time identity of an IDL
 * `any` is by construction not known statically, and a consumer that
 * reaches one defers to the boxed value's `typeId` (DDS-TS 1.0 §7.3.4).
 */
export type DdsTypeRef =
  | { kind: "primitive"; name: PrimitiveName }
  /** A bitset member or a bitmask bit value; `width` is 1..64. */
  | { kind: "bitfield"; width: number }
  | { kind: "string"; bound?: number | undefined; wide: boolean }
  | { kind: "sequence"; element: DdsTypeRef; bound?: number | undefined }
  | { kind: "array"; element: DdsTypeRef; lengths: ReadonlyArray<number> }
  | { kind: "map"; key: DdsTypeRef; value: DdsTypeRef; bound?: number | undefined }
  /** A qualified IDL name; resolve through `lookupType`. */
  | { kind: "ref"; name: string }
  | { kind: "any" };

/**
 * One member of a type, with the annotation metadata that governs it.
 *
 * DDS-TS 1.0 §8.3. `unit`, `min` and `max` are carried for a consumer
 * that wants them and are **not** enforced by the type guard: a guard
 * that rejected an out-of-range value would refuse a sample the wire
 * accepted, which is a different contract from the one the IDL wrote.
 */
export interface DdsMemberDescriptor {
  /** The IDL member name, as declared. */
  readonly name: string;
  /** The effective member id — `@id`, `@autoid`, or the position. */
  readonly id: number;
  /** What the member holds. */
  readonly type: DdsTypeRef;
  /** `@key`. */
  readonly key: boolean;
  /** `@optional`. */
  readonly optional: boolean;
  /** `@must_understand`. */
  readonly mustUnderstand: boolean;
  /** `@default(V)`, or an enumerator's ordinal, or a bit position. */
  readonly default?: unknown;
  /** `@unit("…")` — documentation, never validated. */
  readonly unit?: string | undefined;
  /** `@min(V)` — documentation, never validated. */
  readonly min?: unknown;
  /** `@max(V)` — documentation, never validated. */
  readonly max?: unknown;
  /** `@hashid("…")`, the string the id was hashed from. */
  readonly hashid?: string | undefined;
  /**
   * The case labels that select this member. Union descriptors only
   * (DDS-TS 1.0 §7.5.1); `undefined` marks the `default:` case.
   *
   * `bigint` is one member past the element type §8.3 gives. A
   * `union U switch (long long)` is legal IDL 4.2 and its case labels
   * reach past 2^53, where a `number` holds two of them as one value;
   * the arm's `discriminator` property is a `bigint` for that reason
   * (§7.5), and a label list that could not carry one would disagree
   * with the type it describes. Additive: every label §8.3 admits is
   * still admitted.
   */
  readonly labels?: ReadonlyArray<number | string | boolean | bigint> | undefined;
  /**
   * A map key whose JavaScript equality is not value equality — a
   * struct, an array, a sequence (DDS-TS 1.0 §7.9.1, diagnostic
   * `DDS-TS-W003`). A `Map` keyed by one compares its keys by
   * reference; the flag is what tells a consumer to compare with
   * `equalKey` instead.
   */
  readonly keyEqualityHazard?: boolean | undefined;
  /**
   * The member is the base structure, the first member `parent` (XTypes
   * 1.3 §7.5.1.4.1.3): its type is the `ref` to the base, and the members
   * of that type are members of this one for `getKey` and `withDefaults`.
   */
  readonly base?: boolean | undefined;
}

/**
 * Which IDL declaration a descriptor came from.
 *
 * DDS-TS 1.0 §8.3. `exception` and not a flag on `struct`: an exception
 * maps to the same shape but crosses an Operations-Profile boundary
 * differently, and a consumer switching on `kind` should see the
 * difference.
 */
export type DescriptorKind =
  | "struct"
  | "union"
  | "enum"
  | "bitset"
  | "bitmask"
  | "exception"
  | "alias"
  | "interface";

/**
 * One IDL type, as the value that carries its metadata.
 *
 * DDS-TS 1.0 §8.3. The generated `export const <Type>Type` constant.
 *
 * `typeGuard` is the one member that is not data: a function the
 * generator emits beside the descriptor, checking the presence and type
 * of every **required** member. It does not check `@unit`, `@min` or
 * `@max`, and it does not require optional members.
 */
export interface DdsTypeDescriptor<T = unknown> {
  /** Which IDL declaration this is. */
  readonly kind: DescriptorKind;
  /** The qualified IDL name — `robot::Pose`, not `Pose`. */
  readonly name: string;
  /** `@final` / `@appendable` / `@mutable`, resolved. */
  readonly extensibility: ExtensibilityKind;
  /** `@nested` — whether the type may stand as a topic type. */
  readonly nested: boolean;
  /** `@topic("…")`, where the IDL bound one. */
  readonly topic?: string | undefined;
  /** `@autoid(SEQUENTIAL|HASH)`, where the IDL set one. */
  readonly autoid?: "sequential" | "hash" | undefined;
  /**
   * `@bit_bound(N)`. An enumeration's effective bound (default 32), a
   * bitmask's, or a bitset's total width.
   */
  readonly bitBound?: number | undefined;
  /** A union with a `default:` case (DDS-TS 1.0 §7.5.1). */
  readonly hasDefault?: boolean | undefined;
  /**
   * A union with no `default:` case whose listed cases do not exhaust
   * the discriminator (DDS-TS 1.0 §7.5.2, diagnostic `DDS-TS-W004`).
   */
  readonly hasImplicitDefault?: boolean | undefined;
  /**
   * The members, in declaration order. Empty for an enumeration's
   * holder-only forms and for an `interface`, whose operations live in
   * a Service Descriptor instead.
   */
  readonly fields: ReadonlyArray<DdsMemberDescriptor>;
  /** The emitted `is<Type>` predicate. */
  readonly typeGuard: (v: unknown) => v is T;
}

/**
 * The marker every generated IDL `exception` interface extends.
 *
 * DDS-TS 1.0 §8.3.1. A property and not a class: TypeScript's structural
 * typing has nothing else that separates an exception payload from an
 * ordinary struct at compile time, and this PSM emits no classes.
 */
export interface DdsException {
  readonly __dds_exception: true;
}

/**
 * A boxed IDL `any`: a value together with the identity of its type.
 *
 * DDS-TS 1.0 §8.4 and §7.3.4. `typeId` is the qualified IDL name and
 * indexes the registry, so `lookupType(box.typeId)` answers with the
 * descriptor that describes `value`.
 *
 * Construct with `boxAny`, narrow with `unboxAny` — both in
 * `./dds_any.js`. Reading `value` directly is legal and gives back
 * `unknown`, which is the point: an IDL `any` mapped to TypeScript `any`
 * would switch the checking off rather than carry the identity.
 */
export interface DdsAny {
  /** The qualified IDL name of the boxed value's type. */
  readonly typeId: string;
  /** The payload. `unknown`, and narrowed through the descriptor. */
  readonly value: unknown;
}

/**
 * The type a descriptor guards.
 *
 * DDS-TS 1.0 §8.7. `GuardedTypeOf<typeof PoseType>` is `Pose`, which is
 * what lets `isOneOf` state the union of a list of exception
 * descriptors as one narrowed type.
 */
export type GuardedTypeOf<D> = D extends DdsTypeDescriptor<infer T> ? T : never;
