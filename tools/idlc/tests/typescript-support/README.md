# Fresh TypeScript TypeSupport companions

This fixture consumes current shared-CLI output — `zerodds-idlc generate
--lang typescript`'s `export const <Type>TypeSupport` companion, appended
to the same file as the generated classes and codecs, over
`runtime/typescript`'s `@zerodds/wire`. `support.idl` covers a short
numeric key, a short bounded-string key, a long bounded-string key (past
the sixteen-octet zero-pad/MD5 threshold), an empty aggregate and an
`@appendable` optional value — the same fixture `csharp-support` uses, so
the wire bytes are directly comparable across the two columns.

`crates/ts-node/src/dds.ts` already documents the target shape (`A
TypeSupport as emitted by idlc ts`) and `DomainParticipant.createTypedTopic`
already accepts it; this fixture is what proves a fresh generation actually
produces one.

From the ZeroDDS root, using the already built matching CLI and a built
`@zerodds/wire` (`npm run build` in `runtime/typescript` once):

```sh
generated_support="$(mktemp -d)"
target/debug/zerodds-idlc generate --lang typescript --encoding xcdr2 \
  --out "$generated_support" tools/idlc/tests/typescript-support/support.idl
mkdir -p "$generated_support/node_modules/@zerodds"
ln -s "$PWD/runtime/typescript" "$generated_support/node_modules/@zerodds/wire"
printf '{"type":"module"}' > "$generated_support/package.json"
npx tsx tools/idlc/tests/typescript-support/consumer.ts "$generated_support/support.ts"
```

Repeat generation into another fresh directory with `--encoding xcdr1` for
that representation. The descriptor stays in the generated module — a
`export const <Type>TypeSupport` per topic type, not one table keyed by
name the way Python's `ZD_TOPIC_DESCRIPTORS` is — because a TypeScript
`namespace` already gives every type nested in an IDL module its own
scope (`bridge.PointTypeSupport` beside `bridge.Point`), and a bare
top-level const does the same for one with none.

The consumer checks independent LE/BE octets and key literals, maximum-size
MD5 selection (`bridge::Named`'s `string<32>` key, whose *bound* — not
this run's actual value — is what selects MD5 over zero-padding, checked
against `node:crypto`'s independent digest, never the runtime's own
hand-rolled `md5()`, which is what is under test), optional presence/
absence, an empty type, and four explicit refusals (wrong XCDR
representation on encode and on decode, a truncated body, trailing
octets).

These are codec/descriptor checks. No native DDS endpoint, no `koffi`,
no domain participant is created, and no cross-vendor result is claimed.
Wiring the companion through a live `createTypedTopic` native topic
remains separate integration work — see
[`crates/ts-node/examples/topic-typed-smoke.ts`](../../../../crates/ts-node/examples/topic-typed-smoke.ts)
for the structural (non-native) proof that a generated companion
satisfies `dds.ts`'s `TypeSupport<T>` and reaches `createTypedTopic`'s
adapter unmodified.

`export const <Type>TypeSupport` exposes `typeName`, `isKeyed`,
`extensibility` (0 final, 1 appendable, 2 mutable), `representation` (0
XCDR1, 2 XCDR2 — DDS IDs, matching `zerodds_wire.topic_descriptor.
TopicDescriptor`'s own convention in the Python column), a lazy
`typeObject` getter (throws for a type XTypes cannot lower, e.g. a
union or a `fixed`), `encode`/`decode`, and `keyHash` — present only for
a keyed type, omitted (not `undefined`) for a keyless one, matching
`dds.ts`'s own `keyHash?` optionality.

`encode(sample, endian?, representation?)` and `decode(bytes, offset?,
length?, endian?, representation?)` refuse a `representation` argument
that disagrees with the one representation this particular generation
implements, rather than silently reinterpreting the body under the
wrong framing. `decode`'s `offset`/`length` window a larger buffer
without copying (`bytes.subarray(offset, offset + length)`) — the shape
`crates/ts-node/src/dds.ts`'s `traitsFromTypeSupport` adapter already
calls it with.

Every type's own key-holder helpers are declared once, in one shared
`namespace ZdKeyCodec` covering the whole file — not once per type, and
not appended at the outer scope the way Python's same-named `def`
tolerates: a bare top-level `function zd_enc_p_long` declared twice in
one TypeScript module is `Duplicate function implementation`, which is
why C#'s companion nests its own copy in a `KeyCodec` class and this one
does the same.

## Byte-identical to the Rust reference

`against-recorded-octets.ts` holds the same companion against
`tests/interop-corpus/struct-appendable/` — the Rust column's own
recorded octets in `tests/interop-results/struct-appendable/zerodds-rust/`
— for both XCDR1 and XCDR2, both byte orders, encode AND decode:

```sh
xcdr1_support="$(mktemp -d)"
xcdr2_support="$(mktemp -d)"
target/debug/zerodds-idlc generate --lang typescript --encoding xcdr1 \
  --out "$xcdr1_support" tests/interop-corpus/struct-appendable/type.idl
target/debug/zerodds-idlc generate --lang typescript --encoding xcdr2 \
  --out "$xcdr2_support" tests/interop-corpus/struct-appendable/type.idl
for support in "$xcdr1_support" "$xcdr2_support"; do
  mkdir -p "$support/node_modules/@zerodds"
  ln -s "$PWD/runtime/typescript" "$support/node_modules/@zerodds/wire"
  printf '{"type":"module"}' > "$support/package.json"
done
npx tsx tools/idlc/tests/typescript-support/against-recorded-octets.ts \
  "$xcdr1_support/type.ts" plain-cdr1
npx tsx tools/idlc/tests/typescript-support/against-recorded-octets.ts \
  "$xcdr2_support/type.ts" plain-cdr2
```

Two separate runs, not one importing both generated modules: a generated
module's own top-level `registerType(SampleType)` call (DDS-TS 1.0
chapter 8) refuses a second, structurally different registration under
the same qualified name in one process, and the XCDR1 and XCDR2
generations of the same IDL type are exactly that.

## `createTypedTopic` actually accepts it

`typesupport-interface-check.ts` is type-checked only, never run
(`tsc --noEmit`) — the structural proof that a fresh companion satisfies
`crates/ts-node/src/dds.ts`'s own `TypeSupport<T>`, checked against that
real interface via a type-only import rather than a hand-copied one that
could drift. `npm install` once in `crates/ts-node` (for `koffi`, which
`dds.ts`'s *runtime* code imports unconditionally even though this
check's own import of it is type-only and erased) and:

Generated into this same directory (not a fresh temp one this time —
`typesupport-interface-check.ts`'s `./support.js` and `../../../../
crates/ts-node/src/dds.js` imports are both relative to it) and removed
after, together with the `node_modules`/`package.json` this one check
needs to resolve `@zerodds/wire`:

```sh
cd tools/idlc/tests/typescript-support
../../../../target/debug/zerodds-idlc generate --lang typescript --out . support.idl
mkdir -p node_modules/@zerodds
ln -s "$PWD/../../../../runtime/typescript" node_modules/@zerodds/wire
printf '{"type":"module"}' > package.json
npx tsc --target ES2022 --module NodeNext --moduleResolution NodeNext \
  --strict --skipLibCheck --noEmit typesupport-interface-check.ts support.ts \
  2>&1 | grep -v 'TS2591'
rm -f support.ts package.json && rm -rf node_modules
```

(`TS2591` — "Cannot find name 'Buffer'/'process'", `crates/ts-node/src`'s
own diagnostic for having no `@types/node` dependency, unrelated to this
check — filtered the same way `runtime/typescript/run-tests.sh` filters
it.)

No `koffi`, no domain participant, no native topic — this is the
structural half of "wire `createTypedTopic` to consume it"; a live one is
`crates/ts-node/examples/topic-typed-smoke.ts`'s and
`crates/ts-node/test/smoke.test.ts`'s, over a hand-written
`TypeSupport<Point>` rather than a generated one, and neither creates a
DDS participant on a domain below 180 on a machine where that is the
convention.
