// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

/**
 * The nine descriptor-runtime marks of DDS-TS 1.0 Annex B, §B.2.
 *
 * B.2.1 asks that the package export the chapter-8 names; B.2.2 through
 * B.2.9 ask what each of them does. They are numbered here as the annex
 * numbers them, so a failure names the mark rather than a line.
 *
 * The descriptors below are written by hand and are what the generator
 * emits for the IDL beside each one. Written by hand deliberately: this
 * file measures the **runtime**, and building its inputs by running the
 * generator would mean a generator defect could not fail here without
 * also failing there. `crates/lang-rules/tests/typescript_descriptors.rs`
 * is the other direction — the generator's output, against the same
 * shapes.
 *
 * The exit code carries the class, `docs/specs/zerodds-verdicts-1.0.md`:
 * 0 where every mark held, non-zero for a disagreement with the first
 * line as its evidence.
 */

import {
  boxAny,
  equalKey,
  getKey,
  getTopic,
  isOneOf,
  lookupType,
  makeChar,
  makeLongDouble,
  makeWChar,
  registerType,
  unboxAny,
  withDefaults,
} from "../src/index.js";
import type {
  Char,
  DdsAny,
  DdsException,
  DdsMemberDescriptor,
  DdsTypeDescriptor,
  DdsTypeRef,
  DescriptorKind,
  ExtensibilityKind,
  LongDouble,
  PrimitiveName,
  WChar,
} from "../src/index.js";

// The runtime depends on nothing from Node — it has to run in a browser
// too — so the one global this test needs is declared here rather than
// through a dependency on `@types/node`.
declare const process: { exit(code: number): never };

let checked = 0;
let failures = 0;

/** One mark, with its number. */
function mark(name: string, run: () => void): void {
  checked += 1;
  try {
    run();
  } catch (error) {
    failures += 1;
    console.log(`${name}: ${String(error)}`);
  }
}

/** What has to hold, with what to say when it does not. */
function assert(held: boolean, said: string): void {
  if (!held) {
    throw new Error(said);
  }
}

/** That a call throws, and does not merely return something false. */
function throws(run: () => unknown, said: string): void {
  let threw = false;
  try {
    run();
  } catch {
    threw = true;
  }
  assert(threw, said);
}

// ---------------------------------------------------------------------
// The types the marks are measured over.
//
//   @final @topic("Pose")
//   struct Pose { @key string<32> frame_id; @id(1) double x; @id(2) double y; };
//   @appendable struct Sized { long w; @default(7) long h; };
//   exception NotFound { string what; };
//   exception Denied { long code; };
// ---------------------------------------------------------------------

interface Pose {
  frame_id: string;
  x: number;
  y: number;
}

function isPose(v: unknown): v is Pose {
  return (
    typeof v === "object" &&
    v !== null &&
    typeof (v as Pose).frame_id === "string" &&
    typeof (v as Pose).x === "number" &&
    typeof (v as Pose).y === "number"
  );
}

const PoseType: DdsTypeDescriptor<Pose> = {
  kind: "struct",
  name: "Pose",
  extensibility: "final",
  nested: false,
  topic: "Pose",
  fields: [
    {
      name: "frame_id",
      id: 0,
      type: { kind: "string", bound: 32, wide: false },
      key: true,
      optional: false,
      mustUnderstand: false,
    },
    {
      name: "x",
      id: 1,
      type: { kind: "primitive", name: "double" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
    {
      name: "y",
      id: 2,
      type: { kind: "primitive", name: "double" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
  ],
  typeGuard: isPose,
};

interface Sized {
  w: number;
  h?: number;
}

function isSized(v: unknown): v is Sized {
  return typeof v === "object" && v !== null && typeof (v as Sized).w === "number";
}

const SizedType: DdsTypeDescriptor<Sized> = {
  kind: "struct",
  name: "Sized",
  extensibility: "appendable",
  nested: false,
  fields: [
    {
      name: "w",
      id: 0,
      type: { kind: "primitive", name: "int32" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
    {
      name: "h",
      id: 1,
      type: { kind: "primitive", name: "int32" },
      key: false,
      optional: true,
      mustUnderstand: false,
      default: 7,
    },
  ],
  typeGuard: isSized,
};

interface NotFound extends DdsException {
  what: string;
}

function isNotFound(v: unknown): v is NotFound {
  return typeof v === "object" && v !== null && typeof (v as NotFound).what === "string";
}

const NotFoundType: DdsTypeDescriptor<NotFound> = {
  kind: "exception",
  name: "NotFound",
  extensibility: "appendable",
  nested: false,
  fields: [
    {
      name: "what",
      id: 0,
      type: { kind: "string", wide: false },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
  ],
  typeGuard: isNotFound,
};

interface Denied extends DdsException {
  code: number;
}

function isDenied(v: unknown): v is Denied {
  return typeof v === "object" && v !== null && typeof (v as Denied).code === "number";
}

const DeniedType: DdsTypeDescriptor<Denied> = {
  kind: "exception",
  name: "Denied",
  extensibility: "appendable",
  nested: false,
  fields: [
    {
      name: "code",
      id: 0,
      type: { kind: "primitive", name: "int32" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
  ],
  typeGuard: isDenied,
};

/** `@final struct Keyed { @key Pose at; @key double when; };` */
interface Keyed {
  at: Pose;
  when: number;
}

function isKeyed(v: unknown): v is Keyed {
  return (
    typeof v === "object" &&
    v !== null &&
    isPose((v as Keyed).at) &&
    typeof (v as Keyed).when === "number"
  );
}

const KeyedType: DdsTypeDescriptor<Keyed> = {
  kind: "struct",
  name: "Keyed",
  extensibility: "final",
  nested: false,
  fields: [
    {
      name: "at",
      id: 0,
      type: { kind: "ref", name: "Pose" },
      key: true,
      optional: false,
      mustUnderstand: false,
    },
    {
      name: "when",
      id: 1,
      type: { kind: "primitive", name: "double" },
      key: false,
      optional: false,
      mustUnderstand: false,
    },
  ],
  typeGuard: isKeyed,
};

registerType(PoseType);
registerType(SizedType);
registerType(NotFoundType);
registerType(DeniedType);
registerType(KeyedType);

// ---------------------------------------------------------------------
// B.2.1 — the package exports what chapter 8 defines.
//
// The type half is checked by `tsc`: the tuple below names every type
// the annex lists, and a missing export is a compile error rather than
// a run-time one. The value half is checked here, because a name that
// is exported and is `undefined` compiles perfectly well.
// ---------------------------------------------------------------------

type Exported = [
  DdsTypeDescriptor,
  DdsMemberDescriptor,
  DdsTypeRef,
  DdsAny,
  DdsException,
  DescriptorKind,
  ExtensibilityKind,
  PrimitiveName,
  Char,
  WChar,
  LongDouble,
];

/** How many types the tuple above names. A compile-time count. */
const EXPORTED_TYPES: Exported["length"] = 11;

mark("B.2.1", () => {
  const functions: ReadonlyArray<[string, unknown]> = [
    ["makeChar", makeChar],
    ["makeWChar", makeWChar],
    ["makeLongDouble", makeLongDouble],
    ["registerType", registerType],
    ["lookupType", lookupType],
    ["getKey", getKey],
    ["getTopic", getTopic],
    ["withDefaults", withDefaults],
    ["boxAny", boxAny],
    ["unboxAny", unboxAny],
    ["equalKey", equalKey],
    ["isOneOf", isOneOf],
  ];
  for (const [name, value] of functions) {
    assert(typeof value === "function", `${name} is not exported as a function`);
  }
  assert(EXPORTED_TYPES === 11, "the type list is not the annex's");
  // `getTopic` is `descriptor.topic` unchanged (§8.7), including where
  // the IDL bound none.
  assert(getTopic(PoseType) === "Pose", "the topic binding was lost");
  assert(getTopic(SizedType) === undefined, "a topic was invented");
});

// ---------------------------------------------------------------------
// B.2.2 — `lookupType`.
// ---------------------------------------------------------------------

mark("B.2.2", () => {
  assert(lookupType("Pose") === PoseType, "the registered descriptor did not come back");
  assert(lookupType("robot::Nothing") === undefined, "an unregistered name answered");
  // Re-registering the same descriptor is silent; a different type under
  // a name already taken is not (§8.7).
  registerType(PoseType);
  throws(
    () => {
      registerType({ ...PoseType, extensibility: "mutable" });
    },
    "a second, different `Pose` was accepted",
  );
});

// ---------------------------------------------------------------------
// B.2.3 — `getKey`.
// ---------------------------------------------------------------------

mark("B.2.3", () => {
  const key = getKey({ frame_id: "map", x: 1, y: 2 }, PoseType);
  assert(key.length === 1 && key[0] === "map", `the key is ${JSON.stringify(key)}`);
  const none = getKey({ w: 3 }, SizedType);
  assert(none.length === 0, "a type with no key member answered with one");
  const both = getKey({ at: { frame_id: "a", x: 0, y: 0 }, when: 5 }, KeyedType);
  assert(both.length === 1, "declaration order lost a member");
});

// ---------------------------------------------------------------------
// B.2.4 — `boxAny`.
// ---------------------------------------------------------------------

mark("B.2.4", () => {
  const boxed = boxAny(PoseType, { frame_id: "map", x: 1, y: 2 });
  assert(boxed.typeId === "Pose", `the box says ${boxed.typeId}`);
  assert(isPose(boxed.value), "the box did not carry the value");
  throws(
    () => boxAny(PoseType, { frame_id: "map", x: 1, y: "two" } as unknown as Pose),
    "a value the guard rejects was boxed",
  );
});

// ---------------------------------------------------------------------
// B.2.5 — `unboxAny`.
// ---------------------------------------------------------------------

mark("B.2.5", () => {
  const boxed = boxAny(PoseType, { frame_id: "map", x: 1, y: 2 });
  const back = unboxAny(boxed, PoseType);
  assert(back.frame_id === "map" && back.x === 1, "the value came back changed");
  throws(() => unboxAny(boxed, SizedType), "a box was unwrapped by the wrong descriptor");
  const lying: DdsAny = { typeId: "Pose", value: { frame_id: 1 } };
  throws(() => unboxAny(lying, PoseType), "a box whose value is not its type was accepted");
});

// ---------------------------------------------------------------------
// B.2.6 — `withDefaults`.
// ---------------------------------------------------------------------

mark("B.2.6", () => {
  const filled = withDefaults(SizedType, { w: 3 });
  assert(filled.w === 3, "the supplied member was changed");
  assert(filled.h === 7, `the default did not fill: h is ${String(filled.h)}`);
  const kept = withDefaults(SizedType, { w: 3, h: 9 });
  assert(kept.h === 9, "a supplied member was overwritten by the default");
  // A member with no recorded default stays absent — not present and
  // undefined, absent.
  const bare = withDefaults(PoseType, { frame_id: "map" });
  assert(!("x" in bare), "a member with no default was invented");
});

// ---------------------------------------------------------------------
// B.2.7 — `makeChar` and `makeWChar`.
// ---------------------------------------------------------------------

mark("B.2.7", () => {
  // U+1F600, two UTF-16 code units and one Unicode scalar. A length
  // check on `s.length === 1` rejects this, which is what §7.3.2 names
  // as a MUST NOT.
  assert(makeWChar("\u{1F600}") === "\u{1F600}", "an astral scalar was refused");
  assert(makeChar("ß") === "ß", "U+00DF is in ISO 8859-1 and was refused");
  throws(() => makeChar("€"), "U+20AC is outside ISO 8859-1 and was accepted");
  throws(() => makeChar("ab"), "two code units were accepted as a char");
  throws(() => makeWChar("\uD800"), "an unpaired surrogate was accepted");
  throws(() => makeWChar("ab"), "two scalars were accepted as a wchar");
  throws(() => makeLongDouble(new Uint8Array(8)), "eight octets were accepted as 16");
  assert(makeLongDouble(new Uint8Array(16)).bytes.length === 16, "the octets were changed");
});

// ---------------------------------------------------------------------
// B.2.8 — `equalKey`.
// ---------------------------------------------------------------------

mark("B.2.8", () => {
  const ref: DdsTypeRef = { kind: "ref", name: "Pose" };
  const one: Pose = { frame_id: "map", x: 1, y: 2 };
  const other: Pose = { frame_id: "map", x: 1, y: 2 };
  assert(one !== other, "the two samples are one object, which proves nothing");
  assert(equalKey(one, other, ref), "two structurally equal keys compared unequal");
  assert(
    !equalKey(one, { frame_id: "map", x: 1, y: 3 }, ref),
    "two keys differing in a member compared equal",
  );
  const f64: DdsTypeRef = { kind: "primitive", name: "double" };
  assert(equalKey(Number.NaN, Number.NaN, f64), "NaN did not compare equal to itself");
  assert(equalKey(0, -0, f64), "+0 and -0 compared unequal");
  assert(equalKey(1n, 1n, { kind: "primitive", name: "int64" }), "two bigints compared unequal");
  // A boxed `any` compares its identity and then its payload.
  assert(
    equalKey(boxAny(PoseType, one), boxAny(PoseType, other), { kind: "any" }),
    "two boxes of one value compared unequal",
  );
  assert(
    !equalKey(boxAny(PoseType, one), boxAny(SizedType, { w: 1 }), { kind: "any" }),
    "two boxes of different types compared equal",
  );
  // Not IDL key types, and answering `false` would be a wrong answer
  // rather than a refusal.
  throws(
    () => equalKey([1], [1], { kind: "sequence", element: f64 }),
    "a sequence was compared as a key",
  );
  throws(
    () => equalKey({}, {}, { kind: "ref", name: "Nowhere" }),
    "a ref to an unregistered type answered",
  );
});

// ---------------------------------------------------------------------
// B.2.9 — `isOneOf`.
//
// The rejection an Operations-Profile client produces: an `Error` whose
// `cause` is the decoded IDL exception payload (§11.4).
// ---------------------------------------------------------------------

mark("B.2.9", () => {
  const raised = new Error("NotFound", { cause: { what: "no such topic" } });
  assert(isOneOf(raised, [NotFoundType, DeniedType]), "a raised NotFound was not recognised");
  if (isOneOf(raised, [NotFoundType, DeniedType])) {
    // The narrowing is the point of the type predicate, and this line
    // is where `tsc` checks it.
    const cause: NotFound | Denied = raised.cause;
    assert("what" in cause, "the narrowed cause is not the payload");
  }
  assert(
    !isOneOf(new Error("other", { cause: { unrelated: true } }), [NotFoundType, DeniedType]),
    "an unrelated cause was recognised",
  );
  assert(!isOneOf("not an error", [NotFoundType]), "a string was recognised as a rejection");
  assert(!isOneOf(undefined, [NotFoundType]), "undefined was recognised as a rejection");
});

console.log(`${checked - failures} of ${checked} descriptor-runtime marks held`);
process.exit(failures === 0 ? 0 : 1);
