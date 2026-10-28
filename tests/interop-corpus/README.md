# The interop corpus

The corpus of `docs/specs/zerodds-interop-suite-1.0.md` §3: one directory per
IDL construct, holding the IDL, the top-level type's scoped name, one sample as
JSON, and a README saying what the case would catch.

```text
<case>/
    type.idl        the IDL
    type            the top-level type's scoped name
    values.json     the sample
    README.md       which construct, which form, and what it catches
```

### The evolution cases carry two IDL files

Fourteen cases are named `evolution-*` and hold two more files:

```text
<case>/
    old.idl         the older type, whose members the newer one is a
                    superset of — or is not, where the case is about a
                    pair XTypes 1.3 §7.2.4.4.8 forbids
    old.json        a sample of the older type
```

`old.idl` writes and `type.idl` reads, which is the exchange `@mutable` and
`@appendable` exist for and the one shape a single-type case cannot express: a
new subscriber meeting an old publisher. What the newer type reads back is
recorded in each case's `## Evolution` table and asserted by
`cargo test -p zerodds-wire-probe --test evolution`. A refusal is a recorded
outcome like any other — `evolution-final-tail` is a pair Table 19 forbids, and
the row that says so is the assertion.

Run one case:

```sh
cargo run -q -p zerodds-wire-probe --bin wireprobe-zerodds -- \
    --idl tests/interop-corpus/<case>/type.idl \
    --type "$(cat tests/interop-corpus/<case>/type)" \
    --values tests/interop-corpus/<case>/values.json \
    --encoding xcdr2 --endian le
```

`tests/interop-corpus/run.sh` runs all of them and prints the same verdicts the
table below carries, in all four encoding and byte-order combinations.

The gate is `cargo test -p zerodds-wire-probe --test interop_corpus`, which runs
every case through the reflective encoder in those four combinations and through
all three vendor emitters, and compares the result against each case's own
`## Probe status` table. `run.sh` is the same walk for a human; the test is the
one CI runs, so a case that stops producing the bytes recorded below fails a
build rather than waiting for someone to run a shell script.

### The samples

A sample of zeroes tests nothing, so nothing here is zero unless zero is the
point. Every scalar carries a value that moves if an offset, a width or a byte
order is wrong: negative numbers, values with the top bit set, and adjacent
members that differ from each other. Strings carry multi-byte UTF-8, sequences
appear empty, with one element and with several, and every aggregate has at
least one member after the construct under test so that trailing padding is
visible.

The JSON dialect is the one `crates/wire-probe/src/lib.rs` documents. Two rules
are easy to get wrong and are worth repeating: a 64-bit integer is a **string**,
because a JSON number is a double and loses the low bits past 2^53; and an
absent `@optional` member is a key that is **left out**, never `null`, which the
probe rejects. A `map` is an array of two-element arrays rather than a JSON
object, because a JSON object keys on a string and a map key need not be one.

### Naming a tracker issue in a case README

`tests/interop-results/matrix.html` builds its "Reported issues these rows
answer" section by reading `#<number>` out of these READMEs, and it spreads
what it reads: every number written on a case that draws Cyclone's
no-default-extensibility warning is carried to **every** case that draws it,
which is how `module-issue41-shape` answers for a hundred and fifty. So a
number belongs on the annotated half of a pair and not on its `-unannotated`
twin, unless the report really is about the default. A number written on an
unannotated case is a claim that a hundred and fifty rows answer that report.
A single-digit number counts only when the word `issue` or `issues` is
directly in front of it, because a bare `#6` is a heading far more often than
a citation.

### Why the unannotated twins

Every case whose IDL annotates an extensibility has a `-unannotated` twin
holding the same type with `@final`, `@appendable` and `@mutable` stripped and
nothing else changed. This is not padding: `crates/ast-layout/tests/fixtures/vendor_bytes.md`
§3 records Cyclone's `idlc` reading an unannotated aggregate as final and RTI
Connext reading it as appendable, which under XCDR2 is a four-byte DHEADER in
front of every aggregate in the file. XTypes 1.3 §7.3.1.2.1.8 says appendable.
Diffing a case against its twin isolates that default, because the two files
differ in nothing else. Twins that would have duplicated an existing case — the
cases that are already unannotated — were not generated.

## Coverage against §3 of the interop suite

| §3 line | where |
|---|---|
| primitives, each of the fifteen, alone and mixed | `primitive-*`; fourteen alone, `primitive-longdouble` for the fifteenth, `primitive-mixed` and `primitive-mixed-descending-alignment` |
| `string` / `wstring`, bounded and unbounded, empty and non-empty | `string-*`, `wstring-*`, `string-adjacent-widths` |
| `sequence`, bounded and unbounded, of each element kind, empty and non-empty, nested | `sequence-*`; the primitive element kinds in three cases, one case per constructed element kind, each with an empty, a one-element, a many-element and a bounded member |
| arrays, one, two and three dimensions, of each element kind | `array-*` |
| `enum` at bit bounds 8, 16 and 32, with and without `@value` | `enum-bitbound-{8,16,32}-{plain,value}`, `enum-default-bitbound`, `enum-sparse-values`, `enum-negative-value` |
| `bitmask` at each bit bound, `bitset` | `bitmask-bitbound-{8,16,32,64}`, `bitmask-default-bitbound`, `bitmask-sparse-positions`, `bitset-*` |
| `union` over each legal discriminator kind, with and without `default`, multiple labels per case | `union-discriminator-*` (boolean, char, wchar, octet, int8, uint8, short, ushort, long, ulong, longlong, ulonglong, enum), `union-with-default`, `union-without-default`, `union-default-unselected`, `union-multiple-labels` |
| structures: `@final`, `@appendable`, `@mutable`; inheritance; nested two levels; a `@mutable` inside an `@appendable` | `struct-*` |
| `typedef` chains, including a typedef of a collection | `typedef-*` |
| `map`, of each key and element kind combination | `map-*` |
| `@key` including inside a nested struct, `@optional` present and absent, `@external`, `@must_understand`, `@id`, `@hashid`, `@default` | `annotation-*` |
| module nesting and root-anchored scoped names, the #41 and #42 shape | `module-*` |
| `fixed<P,S>` | `fixed-*` |
| `long double`, as IEEE binary128 | `primitive-longdouble` |
| the unannotated variant of every aggregate | every `*-unannotated` case |

Every construct §3 names now lowers and serialises. `union-discriminator-enum`
and `module-reopened-unqualified` were in the corpus as cases that did not
lower; both do, and what each was blocked on is recorded under the gaps below.

Eleven cases are in no line of §3 and are here because a report put them there.
`identifier-target-keywords`, `identifier-keyword-member-ids`,
`identifier-target-keyword-typename`, `identifier-target-keyword-enumerator`,
`identifier-target-keyword-bitmask-flag` and their twins
come from issue #14: §3 enumerates the language's constructs, and an
identifier that is legal in IDL and reserved in a generated target language is
not a construct — it is a spelling every construct can carry, which is why no
line of §3 reaches it. `evolution-final-to-appendable` comes from issues #41,
#42 and #27: §3 has the two extensibilities separately and the corpus twins
every annotated case against the default, but neither shape puts a `@final`
writer and an `@appendable` reader on the same octets, which is the reported
failure itself.

## What the Rust probe cannot do yet

Two causes are left. Each shows up twice, once in a case and once in its
unannotated twin.

| what | cases | where it lives |
|---|---|---|
| A `bitset` that inherits from another `bitset` is not serialised. The probe refuses the case by name rather than packing it wrongly. | `bitset-inheritance` | `crates/wire-probe/src/encode.rs` |

`long double` used to be the second cause and the reason the table had a
`placeholder` verdict: all three columns wrote a `double` and eight zero
bytes. It is now IEEE binary128 in Rust, C, Python and Java, checked
against GCC's own `__float128` and cross-checked between the four.
`docs/float128-column.md` has the vendor measurements that settled the
representation — none of the four DDS vendors does the same thing, which
is why the clause decides — and `primitive-longdouble/README.md` says
what each of the case's six values catches.

What no column can do for it is compare against a vendor: Cyclone's
`idlc` refuses the type, RTI names no representation for its sixteen
octets, and Fast DDS's bytes differ between x86-64 and aarch64. The
three vendor emitters refuse those cases by name.

### What the five defects were

Running the corpus for the first time turned up five, three of them silent —
output produced, success reported, data left out. Each is fixed, each against a
clause, and each against bytes from Cyclone DDS or RTI Connext 7.7 recorded in
`crates/ast-layout/tests/fixtures/vendor_bytes.md`.

| what | cases | the rule |
|---|---|---|
| **Silent.** Inherited members were not laid out and not written: `Ast::members_of` returns a type's own members, `plan_aggregate` walked that, and `write_aggregate` skipped what the plan did not name. | `struct-inheritance-final`, `-appendable`, `-mutable` | XTypes 1.3 §7.4.1.1.5.1, and §7.3.1.2.1.1 for the member ids continuing across the boundary |
| **Silent.** `typedef long Matrix[2][3];` lowered to a plain alias of `long`; the declarator's dimensions were dropped with no diagnostic. | `typedef-of-array`, `sequence-of-array` | IDL 4.2 §7.4.1.4.4 — one `<declarator>` production, under a `typedef` as on a member |
| **Silent.** A negative `@value` was written as zero: `u32::try_from(-1).unwrap_or(0)`. The case serialised, so it was green in the table and wrong in the bytes. | `enum-negative-value` | XTypes 1.3 §7.4.3.5 rule (5); the ENUM_TYPE holder is Int8, Int16 or Int32, so a negative value is the two's complement |
| An enumerator used as a case label did not resolve, so no union over an enumeration lowered, qualified or not. | `union-discriminator-enum` | IDL 4.2 §7.4.1.4.4.4.2 rule (54) and §7.4.1.4.2 |
| An unqualified reference into a reopened module did not resolve, while the same reference inside a single opening did. | `module-reopened-unqualified` | IDL 4.2 §7.4.2 |

## The other language probes

The verdict column below is the Rust probe's. A second implementation's
verdict per case would double the width of a table that is already three
hundred rows, so each language's result is recorded in aggregate here and
per case in its own gate.

| language | payload runs identical to the Rust probe | where |
|---|---|---|
| Python | 3140 of 3160 — 316 cases, five encodings, both byte orders | `docs/python-column.md`, `cargo test -p zerodds-lang-python` |
| C | 3160 of 3200 — 320 cases, five encodings, both byte orders, and the same 3160 read back as the sample that went in | `docs/c-column.md`, `tools/wireprobe/wireprobe-zerodds-c-corpus` |

The runs missing from each row are `bitset-inheritance` and its
unannotated twin, which every probe refuses for the same reason, and —
for C, which is measured over the two cases added since the Python row
— `primitive-char-above-latin1` and `primitive-wchar-above-bmp`, which
every probe refuses because the code point does not fit the type.
Nothing in the corpus is produced by one probe and refused by another.

Four places where Python and Rust disagree are written up in
`docs/python-column.md`, none of them a payload byte. Ten where C and
Rust did are in `docs/c-column.md`; one of those was a payload byte, one
was a value that came back changed, and one was a KeyHash under a
dialect no corpus case exercises.

## The cases

The verdict column is the result of running the case under
`--encoding xcdr2 --endian le`, not a reading of the source. Each case's own
README carries all four encoding and byte-order combinations, and the KeyHash
for the cases that have a `@key`.

### Primitives

| case | construct | Rust probe |
|---|---|---|
| [`primitive-boolean`](primitive-boolean/) | primitive `boolean` | yes |
| [`primitive-boolean-unannotated`](primitive-boolean-unannotated/) | primitive `boolean`, unannotated | yes |
| [`primitive-char`](primitive-char/) | primitive `char` | yes |
| [`primitive-char-unannotated`](primitive-char-unannotated/) | primitive `char`, unannotated | yes |
| [`primitive-char-above-latin1`](primitive-char-above-latin1/) | primitive `char` holding a code point wider than one byte | no — refused |
| [`primitive-double`](primitive-double/) | primitive `double` | yes |
| [`primitive-double-unannotated`](primitive-double-unannotated/) | primitive `double`, unannotated | yes |
| [`primitive-float`](primitive-float/) | primitive `float` | yes |
| [`primitive-float-unannotated`](primitive-float-unannotated/) | primitive `float`, unannotated | yes |
| [`primitive-int8`](primitive-int8/) | primitive `int8` | yes |
| [`primitive-int8-unannotated`](primitive-int8-unannotated/) | primitive `int8`, unannotated | yes |
| [`primitive-long`](primitive-long/) | primitive `long` | yes |
| [`primitive-long-unannotated`](primitive-long-unannotated/) | primitive `long`, unannotated | yes |
| [`primitive-longdouble`](primitive-longdouble/) | primitive `long double` | placeholder — sixteen bytes, not binary128 |
| [`primitive-longdouble-unannotated`](primitive-longdouble-unannotated/) | primitive `long double`, unannotated | placeholder — sixteen bytes, not binary128 |
| [`primitive-longlong`](primitive-longlong/) | primitive `long long` | yes |
| [`primitive-longlong-unannotated`](primitive-longlong-unannotated/) | primitive `long long`, unannotated | yes |
| [`primitive-mixed`](primitive-mixed/) | every primitive in one aggregate | yes |
| [`primitive-mixed-descending-alignment`](primitive-mixed-descending-alignment/) | every primitive, widest first | yes |
| [`primitive-mixed-descending-alignment-unannotated`](primitive-mixed-descending-alignment-unannotated/) | every primitive, widest first, unannotated | yes |
| [`primitive-mixed-unannotated`](primitive-mixed-unannotated/) | every primitive in one aggregate, unannotated | yes |
| [`primitive-octet`](primitive-octet/) | primitive `octet` | yes |
| [`primitive-octet-unannotated`](primitive-octet-unannotated/) | primitive `octet`, unannotated | yes |
| [`primitive-short`](primitive-short/) | primitive `short` | yes |
| [`primitive-short-unannotated`](primitive-short-unannotated/) | primitive `short`, unannotated | yes |
| [`primitive-uint8`](primitive-uint8/) | primitive `uint8` | yes |
| [`primitive-uint8-unannotated`](primitive-uint8-unannotated/) | primitive `uint8`, unannotated | yes |
| [`primitive-ulong`](primitive-ulong/) | primitive `unsigned long` | yes |
| [`primitive-ulong-unannotated`](primitive-ulong-unannotated/) | primitive `unsigned long`, unannotated | yes |
| [`primitive-ulonglong`](primitive-ulonglong/) | primitive `unsigned long long` | yes |
| [`primitive-ulonglong-unannotated`](primitive-ulonglong-unannotated/) | primitive `unsigned long long`, unannotated | yes |
| [`primitive-ushort`](primitive-ushort/) | primitive `unsigned short` | yes |
| [`primitive-ushort-unannotated`](primitive-ushort-unannotated/) | primitive `unsigned short`, unannotated | yes |
| [`primitive-wchar`](primitive-wchar/) | primitive `wchar` | yes |
| [`primitive-wchar-unannotated`](primitive-wchar-unannotated/) | primitive `wchar`, unannotated | yes |
| [`primitive-wchar-above-bmp`](primitive-wchar-above-bmp/) | primitive `wchar` holding a code point outside the BMP | no — refused |

### Strings and wide strings

| case | construct | Rust probe |
|---|---|---|
| [`string-adjacent-widths`](string-adjacent-widths/) | `string` and `wstring` side by side | yes |
| [`string-adjacent-widths-unannotated`](string-adjacent-widths-unannotated/) | `string` and `wstring` side by side, unannotated | yes |
| [`string-bounded-empty`](string-bounded-empty/) | bounded `string`, empty | yes |
| [`string-bounded-empty-unannotated`](string-bounded-empty-unannotated/) | bounded `string`, empty, unannotated | yes |
| [`string-bounded-utf8`](string-bounded-utf8/) | bounded `string`, multi-byte UTF-8 | yes |
| [`string-bounded-utf8-unannotated`](string-bounded-utf8-unannotated/) | bounded `string`, multi-byte UTF-8, unannotated | yes |
| [`string-unbounded-ascii`](string-unbounded-ascii/) | unbounded `string`, ASCII | yes |
| [`string-unbounded-ascii-unannotated`](string-unbounded-ascii-unannotated/) | unbounded `string`, ASCII, unannotated | yes |
| [`string-unbounded-empty`](string-unbounded-empty/) | unbounded `string`, empty | yes |
| [`string-unbounded-empty-unannotated`](string-unbounded-empty-unannotated/) | unbounded `string`, empty, unannotated | yes |
| [`string-unbounded-utf8`](string-unbounded-utf8/) | unbounded `string`, multi-byte UTF-8 | yes |
| [`string-unbounded-utf8-unannotated`](string-unbounded-utf8-unannotated/) | unbounded `string`, multi-byte UTF-8, unannotated | yes |
| [`wstring-bounded-utf8`](wstring-bounded-utf8/) | bounded `wstring`, multi-byte | yes |
| [`wstring-bounded-utf8-unannotated`](wstring-bounded-utf8-unannotated/) | bounded `wstring`, multi-byte, unannotated | yes |
| [`wstring-unbounded-empty`](wstring-unbounded-empty/) | unbounded `wstring`, empty | yes |
| [`wstring-unbounded-empty-unannotated`](wstring-unbounded-empty-unannotated/) | unbounded `wstring`, empty, unannotated | yes |
| [`wstring-unbounded-utf8`](wstring-unbounded-utf8/) | unbounded `wstring`, multi-byte | yes |
| [`wstring-unbounded-utf8-unannotated`](wstring-unbounded-utf8-unannotated/) | unbounded `wstring`, multi-byte, unannotated | yes |

### Sequences

| case | construct | Rust probe |
|---|---|---|
| [`sequence-bitmask`](sequence-bitmask/) | `sequence<bitmask>` | yes |
| [`sequence-bitmask-unannotated`](sequence-bitmask-unannotated/) | `sequence<bitmask>`, unannotated | yes |
| [`sequence-bitset`](sequence-bitset/) | `sequence<bitset>` | yes |
| [`sequence-bitset-unannotated`](sequence-bitset-unannotated/) | `sequence<bitset>`, unannotated | yes |
| [`sequence-bounded-full`](sequence-bounded-full/) | bounded `sequence` filled to its bound | yes |
| [`sequence-bounded-full-unannotated`](sequence-bounded-full-unannotated/) | bounded `sequence` filled to its bound, unannotated | yes |
| [`sequence-double-and-longdouble`](sequence-double-and-longdouble/) | `sequence<double>` and `sequence<long double>` in one aggregate | placeholder — the `long double` elements are sixteen bytes, not binary128 |
| [`sequence-double-and-longdouble-unannotated`](sequence-double-and-longdouble-unannotated/) | `sequence<double>` and `sequence<long double>` in one aggregate, unannotated | placeholder — the `long double` elements are sixteen bytes, not binary128 |
| [`sequence-enum`](sequence-enum/) | `sequence<enum>` | yes |
| [`sequence-enum-unannotated`](sequence-enum-unannotated/) | `sequence<enum>`, unannotated | yes |
| [`sequence-fixed`](sequence-fixed/) | `sequence<fixed>` | yes |
| [`sequence-fixed-unannotated`](sequence-fixed-unannotated/) | `sequence<fixed>`, unannotated | yes |
| [`sequence-map`](sequence-map/) | `sequence<map>` | yes |
| [`sequence-map-unannotated`](sequence-map-unannotated/) | `sequence<map>`, unannotated | yes |
| [`sequence-nested`](sequence-nested/) | `sequence<sequence<long>>` | yes |
| [`sequence-nested-struct`](sequence-nested-struct/) | `sequence<sequence<struct>>` | yes |
| [`sequence-nested-struct-unannotated`](sequence-nested-struct-unannotated/) | `sequence<sequence<struct>>`, unannotated | yes |
| [`sequence-nested-unannotated`](sequence-nested-unannotated/) | `sequence<sequence<long>>`, unannotated | yes |
| [`sequence-of-array`](sequence-of-array/) | `sequence` of an array | yes |
| [`sequence-of-array-unannotated`](sequence-of-array-unannotated/) | `sequence` of an array, unannotated | yes |
| [`sequence-of-typedef`](sequence-of-typedef/) | `sequence` of a typedef chain | yes |
| [`sequence-of-typedef-unannotated`](sequence-of-typedef-unannotated/) | `sequence` of a typedef chain, unannotated | yes |
| [`sequence-primitives-bounded`](sequence-primitives-bounded/) | bounded `sequence` of each primitive kind | yes |
| [`sequence-primitives-bounded-unannotated`](sequence-primitives-bounded-unannotated/) | bounded `sequence` of each primitive kind, unannotated | yes |
| [`sequence-primitives-empty`](sequence-primitives-empty/) | `sequence` of each primitive kind, empty | yes |
| [`sequence-primitives-empty-unannotated`](sequence-primitives-empty-unannotated/) | `sequence` of each primitive kind, empty, unannotated | yes |
| [`sequence-primitives-nonempty`](sequence-primitives-nonempty/) | `sequence` of each primitive kind, non-empty | yes |
| [`sequence-primitives-nonempty-unannotated`](sequence-primitives-nonempty-unannotated/) | `sequence` of each primitive kind, non-empty, unannotated | yes |
| [`sequence-string`](sequence-string/) | `sequence<string>` | yes |
| [`sequence-string-unannotated`](sequence-string-unannotated/) | `sequence<string>`, unannotated | yes |
| [`sequence-struct-appendable`](sequence-struct-appendable/) | `sequence` of a appendable structure | yes |
| [`sequence-struct-final`](sequence-struct-final/) | `sequence` of a final structure | yes |
| [`sequence-struct-final-unannotated`](sequence-struct-final-unannotated/) | `sequence` of a final structure, unannotated | yes |
| [`sequence-struct-mutable`](sequence-struct-mutable/) | `sequence` of a mutable structure | yes |
| [`sequence-struct-unannotated`](sequence-struct-unannotated/) | `sequence` of a unannotated structure | yes |
| [`sequence-union`](sequence-union/) | `sequence<union>` | yes |
| [`sequence-union-unannotated`](sequence-union-unannotated/) | `sequence<union>`, unannotated | yes |
| [`sequence-wstring`](sequence-wstring/) | `sequence<wstring>` | yes |
| [`sequence-wstring-unannotated`](sequence-wstring-unannotated/) | `sequence<wstring>`, unannotated | yes |

### Arrays

| case | construct | Rust probe |
|---|---|---|
| [`array-1d-bitmask`](array-1d-bitmask/) | one-dimensional array of `bitmask` | yes |
| [`array-1d-bitmask-unannotated`](array-1d-bitmask-unannotated/) | one-dimensional array of `bitmask`, unannotated | yes |
| [`array-1d-bitset`](array-1d-bitset/) | one-dimensional array of `bitset` | yes |
| [`array-1d-bitset-unannotated`](array-1d-bitset-unannotated/) | one-dimensional array of `bitset`, unannotated | yes |
| [`array-1d-enum`](array-1d-enum/) | one-dimensional array of `enum` | yes |
| [`array-1d-enum-unannotated`](array-1d-enum-unannotated/) | one-dimensional array of `enum`, unannotated | yes |
| [`array-1d-fixed`](array-1d-fixed/) | one-dimensional array of `fixed` | yes |
| [`array-1d-fixed-unannotated`](array-1d-fixed-unannotated/) | one-dimensional array of `fixed`, unannotated | yes |
| [`array-1d-map`](array-1d-map/) | one-dimensional array of `map` | yes |
| [`array-1d-map-unannotated`](array-1d-map-unannotated/) | one-dimensional array of `map`, unannotated | yes |
| [`array-1d-primitives`](array-1d-primitives/) | one-dimensional array of each primitive kind | yes |
| [`array-1d-primitives-unannotated`](array-1d-primitives-unannotated/) | one-dimensional array of each primitive kind, unannotated | yes |
| [`array-1d-sequence`](array-1d-sequence/) | one-dimensional array of `sequence` | yes |
| [`array-1d-sequence-unannotated`](array-1d-sequence-unannotated/) | one-dimensional array of `sequence`, unannotated | yes |
| [`array-1d-string`](array-1d-string/) | one-dimensional array of `string` | yes |
| [`array-1d-string-unannotated`](array-1d-string-unannotated/) | one-dimensional array of `string`, unannotated | yes |
| [`array-1d-struct-appendable`](array-1d-struct-appendable/) | one-dimensional array of a appendable structure | yes |
| [`array-1d-struct-final`](array-1d-struct-final/) | one-dimensional array of a final structure | yes |
| [`array-1d-struct-final-unannotated`](array-1d-struct-final-unannotated/) | one-dimensional array of a final structure, unannotated | yes |
| [`array-1d-struct-mutable`](array-1d-struct-mutable/) | one-dimensional array of a mutable structure | yes |
| [`array-1d-struct-unannotated`](array-1d-struct-unannotated/) | one-dimensional array of a unannotated structure | yes |
| [`array-1d-union`](array-1d-union/) | one-dimensional array of `union` | yes |
| [`array-1d-union-unannotated`](array-1d-union-unannotated/) | one-dimensional array of `union`, unannotated | yes |
| [`array-1d-wstring`](array-1d-wstring/) | one-dimensional array of `wstring` | yes |
| [`array-1d-wstring-unannotated`](array-1d-wstring-unannotated/) | one-dimensional array of `wstring`, unannotated | yes |
| [`array-2d-primitives`](array-2d-primitives/) | two-dimensional array of each primitive kind | yes |
| [`array-2d-primitives-unannotated`](array-2d-primitives-unannotated/) | two-dimensional array of each primitive kind, unannotated | yes |
| [`array-2d-string`](array-2d-string/) | two-dimensional array of `string` | yes |
| [`array-2d-string-unannotated`](array-2d-string-unannotated/) | two-dimensional array of `string`, unannotated | yes |
| [`array-2d-struct`](array-2d-struct/) | two-dimensional array of a structure | yes |
| [`array-2d-struct-unannotated`](array-2d-struct-unannotated/) | two-dimensional array of a structure, unannotated | yes |
| [`array-3d-enum`](array-3d-enum/) | three-dimensional array of `enum` | yes |
| [`array-3d-enum-unannotated`](array-3d-enum-unannotated/) | three-dimensional array of `enum`, unannotated | yes |
| [`array-3d-primitives`](array-3d-primitives/) | three-dimensional array of each primitive kind | yes |
| [`array-3d-primitives-unannotated`](array-3d-primitives-unannotated/) | three-dimensional array of each primitive kind, unannotated | yes |
| [`array-3d-struct`](array-3d-struct/) | three-dimensional array of a structure | yes |
| [`array-3d-struct-unannotated`](array-3d-struct-unannotated/) | three-dimensional array of a structure, unannotated | yes |
| [`array-odd-element`](array-odd-element/) | array of an odd-sized structure | yes |
| [`array-odd-element-unannotated`](array-odd-element-unannotated/) | array of an odd-sized structure, unannotated | yes |

### Enumerations

| case | construct | Rust probe |
|---|---|---|
| [`enum-bitbound-16-plain`](enum-bitbound-16-plain/) | `enum` at bit bound 16, implicit values | yes |
| [`enum-bitbound-16-plain-unannotated`](enum-bitbound-16-plain-unannotated/) | `enum` at bit bound 16, implicit values, unannotated | yes |
| [`enum-bitbound-16-value`](enum-bitbound-16-value/) | `enum` at bit bound 16, explicit `@value` | yes |
| [`enum-bitbound-16-value-unannotated`](enum-bitbound-16-value-unannotated/) | `enum` at bit bound 16, explicit `@value`, unannotated | yes |
| [`enum-bitbound-32-plain`](enum-bitbound-32-plain/) | `enum` at bit bound 32, implicit values | yes |
| [`enum-bitbound-32-plain-unannotated`](enum-bitbound-32-plain-unannotated/) | `enum` at bit bound 32, implicit values, unannotated | yes |
| [`enum-bitbound-32-value`](enum-bitbound-32-value/) | `enum` at bit bound 32, explicit `@value` | yes |
| [`enum-bitbound-32-value-unannotated`](enum-bitbound-32-value-unannotated/) | `enum` at bit bound 32, explicit `@value`, unannotated | yes |
| [`enum-bitbound-8-plain`](enum-bitbound-8-plain/) | `enum` at bit bound 8, implicit values | yes |
| [`enum-bitbound-8-plain-unannotated`](enum-bitbound-8-plain-unannotated/) | `enum` at bit bound 8, implicit values, unannotated | yes |
| [`enum-bitbound-8-value`](enum-bitbound-8-value/) | `enum` at bit bound 8, explicit `@value` | yes |
| [`enum-bitbound-8-value-unannotated`](enum-bitbound-8-value-unannotated/) | `enum` at bit bound 8, explicit `@value`, unannotated | yes |
| [`enum-default-bitbound`](enum-default-bitbound/) | `enum` with no `@bit_bound` | yes |
| [`enum-default-bitbound-unannotated`](enum-default-bitbound-unannotated/) | `enum` with no `@bit_bound`, unannotated | yes |
| [`enum-negative-value`](enum-negative-value/) | `enum` with a negative `@value` | yes |
| [`enum-negative-value-unannotated`](enum-negative-value-unannotated/) | `enum` with a negative `@value`, unannotated | yes |
| [`enum-sparse-values`](enum-sparse-values/) | `enum` with non-contiguous `@value` | yes |
| [`enum-sparse-values-unannotated`](enum-sparse-values-unannotated/) | `enum` with non-contiguous `@value`, unannotated | yes |

### Bitmask and bitset

| case | construct | Rust probe |
|---|---|---|
| [`bitmask-bitbound-16`](bitmask-bitbound-16/) | `bitmask` at bit bound 16 | yes |
| [`bitmask-bitbound-16-unannotated`](bitmask-bitbound-16-unannotated/) | `bitmask` at bit bound 16, unannotated | yes |
| [`bitmask-bitbound-32`](bitmask-bitbound-32/) | `bitmask` at bit bound 32 | yes |
| [`bitmask-bitbound-32-unannotated`](bitmask-bitbound-32-unannotated/) | `bitmask` at bit bound 32, unannotated | yes |
| [`bitmask-bitbound-64`](bitmask-bitbound-64/) | `bitmask` at bit bound 64 | yes |
| [`bitmask-bitbound-64-unannotated`](bitmask-bitbound-64-unannotated/) | `bitmask` at bit bound 64, unannotated | yes |
| [`bitmask-bitbound-8`](bitmask-bitbound-8/) | `bitmask` at bit bound 8 | yes |
| [`bitmask-bitbound-8-unannotated`](bitmask-bitbound-8-unannotated/) | `bitmask` at bit bound 8, unannotated | yes |
| [`bitmask-default-bitbound`](bitmask-default-bitbound/) | `bitmask` with no `@bit_bound` | yes |
| [`bitmask-default-bitbound-unannotated`](bitmask-default-bitbound-unannotated/) | `bitmask` with no `@bit_bound`, unannotated | yes |
| [`bitmask-sparse-positions`](bitmask-sparse-positions/) | `bitmask` with implicit and explicit positions | yes |
| [`bitmask-sparse-positions-unannotated`](bitmask-sparse-positions-unannotated/) | `bitmask` with implicit and explicit positions, unannotated | yes |
| [`bitset-basic`](bitset-basic/) | `bitset` with two fields | yes |
| [`bitset-basic-unannotated`](bitset-basic-unannotated/) | `bitset` with two fields, unannotated | yes |
| [`bitset-inheritance`](bitset-inheritance/) | `bitset` extending another `bitset` | no — bitset inheritance is not serialised yet |
| [`bitset-inheritance-unannotated`](bitset-inheritance-unannotated/) | `bitset` extending another `bitset`, unannotated | no — bitset inheritance is not serialised yet |
| [`bitset-multiword`](bitset-multiword/) | `bitset` wider than one word | yes |
| [`bitset-multiword-unannotated`](bitset-multiword-unannotated/) | `bitset` wider than one word, unannotated | yes |
| [`bitset-typed-fields`](bitset-typed-fields/) | `bitset` with declared field types | yes |
| [`bitset-typed-fields-unannotated`](bitset-typed-fields-unannotated/) | `bitset` with declared field types, unannotated | yes |

### Unions

| case | construct | Rust probe |
|---|---|---|
| [`union-appendable`](union-appendable/) | appendable `union` | yes |
| [`union-default-unselected`](union-default-unselected/) | `union` with no `default`, discriminator matching nothing | yes |
| [`union-default-unselected-unannotated`](union-default-unselected-unannotated/) | `union` with no `default`, discriminator matching nothing, unannotated | yes |
| [`union-discriminator-boolean`](union-discriminator-boolean/) | `union` over `boolean` | yes |
| [`union-discriminator-boolean-unannotated`](union-discriminator-boolean-unannotated/) | `union` over `boolean`, unannotated | yes |
| [`union-discriminator-char`](union-discriminator-char/) | `union` over `char` | yes |
| [`union-discriminator-char-unannotated`](union-discriminator-char-unannotated/) | `union` over `char`, unannotated | yes |
| [`union-discriminator-enum`](union-discriminator-enum/) | `union` over an `enum` | yes |
| [`union-discriminator-enum-unannotated`](union-discriminator-enum-unannotated/) | `union` over an `enum`, unannotated | yes |
| [`union-discriminator-int8`](union-discriminator-int8/) | `union` over `int8` | yes |
| [`union-discriminator-int8-unannotated`](union-discriminator-int8-unannotated/) | `union` over `int8`, unannotated | yes |
| [`union-discriminator-long`](union-discriminator-long/) | `union` over `long` | yes |
| [`union-discriminator-long-unannotated`](union-discriminator-long-unannotated/) | `union` over `long`, unannotated | yes |
| [`union-discriminator-longlong`](union-discriminator-longlong/) | `union` over `long long` | yes |
| [`union-discriminator-longlong-unannotated`](union-discriminator-longlong-unannotated/) | `union` over `long long`, unannotated | yes |
| [`union-discriminator-octet`](union-discriminator-octet/) | `union` over `octet` | yes |
| [`union-discriminator-octet-unannotated`](union-discriminator-octet-unannotated/) | `union` over `octet`, unannotated | yes |
| [`union-discriminator-short`](union-discriminator-short/) | `union` over `short` | yes |
| [`union-discriminator-short-unannotated`](union-discriminator-short-unannotated/) | `union` over `short`, unannotated | yes |
| [`union-discriminator-uint8`](union-discriminator-uint8/) | `union` over `uint8` | yes |
| [`union-discriminator-uint8-unannotated`](union-discriminator-uint8-unannotated/) | `union` over `uint8`, unannotated | yes |
| [`union-discriminator-ulong`](union-discriminator-ulong/) | `union` over `unsigned long` | yes |
| [`union-discriminator-ulong-unannotated`](union-discriminator-ulong-unannotated/) | `union` over `unsigned long`, unannotated | yes |
| [`union-discriminator-ulonglong`](union-discriminator-ulonglong/) | `union` over `unsigned long long` | yes |
| [`union-discriminator-ulonglong-unannotated`](union-discriminator-ulonglong-unannotated/) | `union` over `unsigned long long`, unannotated | yes |
| [`union-discriminator-ushort`](union-discriminator-ushort/) | `union` over `unsigned short` | yes |
| [`union-discriminator-ushort-unannotated`](union-discriminator-ushort-unannotated/) | `union` over `unsigned short`, unannotated | yes |
| [`union-discriminator-wchar`](union-discriminator-wchar/) | `union` over `wchar` | yes |
| [`union-discriminator-wchar-unannotated`](union-discriminator-wchar-unannotated/) | `union` over `wchar`, unannotated | yes |
| [`union-final`](union-final/) | final `union` | yes |
| [`union-final-unannotated`](union-final-unannotated/) | final `union`, unannotated | yes |
| [`union-member-sequence`](union-member-sequence/) | `union` with a sequence branch | yes |
| [`union-member-sequence-unannotated`](union-member-sequence-unannotated/) | `union` with a sequence branch, unannotated | yes |
| [`union-member-struct`](union-member-struct/) | `union` with an aggregate branch | yes |
| [`union-member-struct-unannotated`](union-member-struct-unannotated/) | `union` with an aggregate branch, unannotated | yes |
| [`union-multiple-labels`](union-multiple-labels/) | `union` with several labels on one case | yes |
| [`union-multiple-labels-unannotated`](union-multiple-labels-unannotated/) | `union` with several labels on one case, unannotated | yes |
| [`union-mutable`](union-mutable/) | mutable `union` | yes |
| [`union-with-default`](union-with-default/) | `union` with a `default` branch, default selected | yes |
| [`union-with-default-unannotated`](union-with-default-unannotated/) | `union` with a `default` branch, default selected, unannotated | yes |
| [`union-without-default`](union-without-default/) | `union` with no `default` | yes |
| [`union-without-default-unannotated`](union-without-default-unannotated/) | `union` with no `default`, unannotated | yes |

### Structures

| case | construct | Rust probe |
|---|---|---|
| [`struct-appendable`](struct-appendable/) | appendable structure | yes |
| [`struct-appendable-in-mutable`](struct-appendable-in-mutable/) | `@appendable` inside `@mutable` | yes |
| [`struct-final`](struct-final/) | final structure | yes |
| [`struct-final-in-mutable`](struct-final-in-mutable/) | `@final` inside `@mutable` | yes |
| [`struct-inheritance-appendable`](struct-inheritance-appendable/) | appendable structure with a base | yes |
| [`struct-inheritance-final`](struct-inheritance-final/) | final structure with a base | yes |
| [`struct-inheritance-final-unannotated`](struct-inheritance-final-unannotated/) | final structure with a base, unannotated | yes |
| [`struct-inheritance-mutable`](struct-inheritance-mutable/) | mutable structure with a base | yes |
| [`struct-many-members`](struct-many-members/) | structure with many members of one kind | yes |
| [`struct-many-members-unannotated`](struct-many-members-unannotated/) | structure with many members of one kind, unannotated | yes |
| [`struct-mutable`](struct-mutable/) | mutable structure | yes |
| [`struct-mutable-length-codes`](struct-mutable-length-codes/) | every EMHEADER length code a `@mutable` member can take | yes |
| [`struct-mutable-in-appendable`](struct-mutable-in-appendable/) | `@mutable` inside `@appendable` | yes |
| [`struct-mutable-in-appendable-unannotated`](struct-mutable-in-appendable-unannotated/) | `@mutable` inside `@appendable`, unannotated | yes |
| [`struct-nested-two-deep`](struct-nested-two-deep/) | structure nested two levels | yes |
| [`struct-nested-two-deep-unannotated`](struct-nested-two-deep-unannotated/) | structure nested two levels, unannotated | yes |
| [`struct-no-trailing-padding`](struct-no-trailing-padding/) | structure that does not pad to its own alignment | yes |
| [`struct-no-trailing-padding-unannotated`](struct-no-trailing-padding-unannotated/) | structure that does not pad to its own alignment, unannotated | yes |
| [`struct-single-member`](struct-single-member/) | structure with one member | yes |
| [`struct-single-member-unannotated`](struct-single-member-unannotated/) | structure with one member, unannotated | yes |
| [`struct-unannotated`](struct-unannotated/) | structure with no extensibility annotation | yes |

### Typedefs

| case | construct | Rust probe |
|---|---|---|
| [`typedef-anonymous-collection-member`](typedef-anonymous-collection-member/) | array declared on the member | yes |
| [`typedef-anonymous-collection-member-unannotated`](typedef-anonymous-collection-member-unannotated/) | array declared on the member, unannotated | yes |
| [`typedef-chain-primitive`](typedef-chain-primitive/) | `typedef` chain over a primitive | yes |
| [`typedef-chain-primitive-unannotated`](typedef-chain-primitive-unannotated/) | `typedef` chain over a primitive, unannotated | yes |
| [`typedef-of-array`](typedef-of-array/) | `typedef` of an array | yes |
| [`typedef-of-array-unannotated`](typedef-of-array-unannotated/) | `typedef` of an array, unannotated | yes |
| [`typedef-of-map`](typedef-of-map/) | `typedef` of a map | yes |
| [`typedef-of-map-unannotated`](typedef-of-map-unannotated/) | `typedef` of a map, unannotated | yes |
| [`typedef-of-sequence`](typedef-of-sequence/) | `typedef` of a sequence | yes |
| [`typedef-of-sequence-unannotated`](typedef-of-sequence-unannotated/) | `typedef` of a sequence, unannotated | yes |
| [`typedef-of-string`](typedef-of-string/) | `typedef` of a bounded string | yes |
| [`typedef-of-string-unannotated`](typedef-of-string-unannotated/) | `typedef` of a bounded string, unannotated | yes |
| [`typedef-of-struct`](typedef-of-struct/) | `typedef` of a structure | yes |
| [`typedef-of-struct-unannotated`](typedef-of-struct-unannotated/) | `typedef` of a structure, unannotated | yes |
| [`typedef-of-union`](typedef-of-union/) | `typedef` of a union | yes |
| [`typedef-of-union-unannotated`](typedef-of-union-unannotated/) | `typedef` of a union, unannotated | yes |

### Maps

| case | construct | Rust probe |
|---|---|---|
| [`map-bounded`](map-bounded/) | bounded `map` | yes |
| [`map-bounded-unannotated`](map-bounded-unannotated/) | bounded `map`, unannotated | yes |
| [`map-empty`](map-empty/) | empty `map` | yes |
| [`map-empty-unannotated`](map-empty-unannotated/) | empty `map`, unannotated | yes |
| [`map-key-boolean`](map-key-boolean/) | `map` keyed by `boolean` | yes |
| [`map-key-boolean-unannotated`](map-key-boolean-unannotated/) | `map` keyed by `boolean`, unannotated | yes |
| [`map-key-char`](map-key-char/) | `map` keyed by `char` | yes |
| [`map-key-char-unannotated`](map-key-char-unannotated/) | `map` keyed by `char`, unannotated | yes |
| [`map-key-enum`](map-key-enum/) | `map` keyed by an `enum` | yes |
| [`map-key-enum-unannotated`](map-key-enum-unannotated/) | `map` keyed by an `enum`, unannotated | yes |
| [`map-key-long`](map-key-long/) | `map` keyed by `long` | yes |
| [`map-key-long-unannotated`](map-key-long-unannotated/) | `map` keyed by `long`, unannotated | yes |
| [`map-key-string`](map-key-string/) | `map` keyed by `string` | yes |
| [`map-key-string-unannotated`](map-key-string-unannotated/) | `map` keyed by `string`, unannotated | yes |
| [`map-key-ulonglong`](map-key-ulonglong/) | `map` keyed by `unsigned long long` | yes |
| [`map-key-ulonglong-unannotated`](map-key-ulonglong-unannotated/) | `map` keyed by `unsigned long long`, unannotated | yes |
| [`map-nested`](map-nested/) | `map` with a map value | yes |
| [`map-nested-unannotated`](map-nested-unannotated/) | `map` with a map value, unannotated | yes |
| [`map-value-sequence`](map-value-sequence/) | `map` with a sequence value | yes |
| [`map-value-sequence-unannotated`](map-value-sequence-unannotated/) | `map` with a sequence value, unannotated | yes |
| [`map-value-string`](map-value-string/) | `map` with a string value | yes |
| [`map-value-string-unannotated`](map-value-string-unannotated/) | `map` with a string value, unannotated | yes |
| [`map-value-struct`](map-value-struct/) | `map` with a structure value | yes |
| [`map-value-struct-unannotated`](map-value-struct-unannotated/) | `map` with a structure value, unannotated | yes |

### Annotations

| case | construct | Rust probe |
|---|---|---|
| [`annotation-default-value`](annotation-default-value/) | `@default` | yes |
| [`annotation-scoped-name`](annotation-scoped-name/) | an annotation applied by a scoped name — `@::key` and `@ann::mark(7)`, §8.2 rule (218) | yes |
| [`annotation-default-value-unannotated`](annotation-default-value-unannotated/) | `@default`, unannotated | yes |
| [`annotation-external-optional-absent`](annotation-external-optional-absent/) | `@optional @external` on one member, left out | yes |
| [`annotation-external-optional-present`](annotation-external-optional-present/) | `@optional @external` on one member, with a value | yes |
| [`annotation-external-primitive`](annotation-external-primitive/) | `@external` on a primitive member | yes |
| [`annotation-external-primitive-unannotated`](annotation-external-primitive-unannotated/) | `@external` on a primitive member, unannotated | yes |
| [`annotation-external-sequence`](annotation-external-sequence/) | `@external` on a sequence and on a string member | yes |
| [`annotation-external-struct`](annotation-external-struct/) | `@external` on an aggregate member | yes |
| [`annotation-external-struct-unannotated`](annotation-external-struct-unannotated/) | `@external` on an aggregate member, unannotated | yes |
| [`annotation-external-typedef`](annotation-external-typedef/) | `@external` on a member whose type is a typedef | yes |
| [`annotation-external-union-branch`](annotation-external-union-branch/) | `@external` on a union branch | yes |
| [`annotation-external-union-branch-unannotated`](annotation-external-union-branch-unannotated/) | `@external` on a union branch, unannotated | yes |
| [`annotation-hashid-implicit`](annotation-hashid-implicit/) | `@hashid` with no argument | yes |
| [`annotation-hashid-implicit-unannotated`](annotation-hashid-implicit-unannotated/) | `@hashid` with no argument, unannotated | yes |
| [`annotation-hashid-named`](annotation-hashid-named/) | `@hashid("name")` | yes |
| [`annotation-hashid-named-unannotated`](annotation-hashid-named-unannotated/) | `@hashid("name")`, unannotated | yes |
| [`annotation-id-explicit`](annotation-id-explicit/) | `@id` | yes |
| [`annotation-id-explicit-unannotated`](annotation-id-explicit-unannotated/) | `@id`, unannotated | yes |
| [`annotation-id-sparse`](annotation-id-sparse/) | `@id` with gaps and a large value | yes |
| [`annotation-id-sparse-unannotated`](annotation-id-sparse-unannotated/) | `@id` with gaps and a large value, unannotated | yes |
| [`annotation-key-multiple`](annotation-key-multiple/) | `@key` on several members | yes |
| [`annotation-key-multiple-unannotated`](annotation-key-multiple-unannotated/) | `@key` on several members, unannotated | yes |
| [`annotation-key-nested-struct`](annotation-key-nested-struct/) | `@key` inside a nested structure | yes |
| [`annotation-key-nested-struct-unannotated`](annotation-key-nested-struct-unannotated/) | `@key` inside a nested structure, unannotated | yes |
| [`annotation-key-single`](annotation-key-single/) | `@key` on one member | yes |
| [`annotation-key-single-unannotated`](annotation-key-single-unannotated/) | `@key` on one member, unannotated | yes |
| [`annotation-key-whole-struct`](annotation-key-whole-struct/) | `@key` on an unkeyed nested structure | yes |
| [`annotation-key-whole-struct-unannotated`](annotation-key-whole-struct-unannotated/) | `@key` on an unkeyed nested structure, unannotated | yes |
| [`annotation-must-understand`](annotation-must-understand/) | `@must_understand` | yes |
| [`annotation-must-understand-unannotated`](annotation-must-understand-unannotated/) | `@must_understand`, unannotated | yes |
| [`annotation-optional-absent`](annotation-optional-absent/) | `@optional` left out | yes |
| [`annotation-optional-absent-unannotated`](annotation-optional-absent-unannotated/) | `@optional` left out, unannotated | yes |
| [`annotation-optional-mixed`](annotation-optional-mixed/) | several `@optional` members, some absent | yes |
| [`annotation-optional-mixed-unannotated`](annotation-optional-mixed-unannotated/) | several `@optional` members, some absent, unannotated | yes |
| [`annotation-optional-present`](annotation-optional-present/) | `@optional` with a value | yes |
| [`annotation-optional-present-unannotated`](annotation-optional-present-unannotated/) | `@optional` with a value, unannotated | yes |

### Modules and scoped names

| case | construct | Rust probe |
|---|---|---|
| [`module-issue41-shape`](module-issue41-shape/) | the unannotated #41 and #42 shape | yes |
| [`module-nesting`](module-nesting/) | modules nested two deep | yes |
| [`module-nesting-unannotated`](module-nesting-unannotated/) | modules nested two deep, unannotated | yes |
| [`module-reopened`](module-reopened/) | a module reopened | yes |
| [`module-reopened-unannotated`](module-reopened-unannotated/) | a module reopened, unannotated | yes |
| [`module-reopened-unqualified`](module-reopened-unqualified/) | a module reopened, unqualified reference | yes |
| [`module-reopened-unqualified-unannotated`](module-reopened-unqualified-unannotated/) | a module reopened, unqualified reference, unannotated | yes |
| [`module-root-anchored`](module-root-anchored/) | root-anchored scoped names | yes |

### Identifiers

| case | construct | Rust probe |
|---|---|---|
| [`identifier-target-keywords`](identifier-target-keywords/) | identifiers legal in IDL and reserved in a generated target language | yes |
| [`identifier-target-keywords-unannotated`](identifier-target-keywords-unannotated/) | the same, unannotated | yes |
| [`identifier-keyword-member-ids`](identifier-keyword-member-ids/) | member ids and the KeyHash from the IDL spelling of a keyword name | yes |
| [`identifier-keyword-member-ids-unannotated`](identifier-keyword-member-ids-unannotated/) | the same, unannotated | yes |
| [`identifier-target-keyword-typename`](identifier-target-keyword-typename/) | a type name that is a keyword of the generated target language | yes |
| [`identifier-target-keyword-typename-unannotated`](identifier-target-keyword-typename-unannotated/) | the same, unannotated | yes |
| [`identifier-target-keyword-enumerator`](identifier-target-keyword-enumerator/) | enumerators that are keywords of the generated target language | yes |
| [`identifier-target-keyword-enumerator-unannotated`](identifier-target-keyword-enumerator-unannotated/) | the same, unannotated | yes |
| [`identifier-target-keyword-bitmask-flag`](identifier-target-keyword-bitmask-flag/) | bitmask flags that are keywords of the generated target language | yes |
| [`identifier-target-keyword-bitmask-flag-unannotated`](identifier-target-keyword-bitmask-flag-unannotated/) | the same, unannotated | yes |

The three cases beyond `identifier-target-keywords` are the identifier
positions its own IDL cannot reach. Everything it declares is inside `module
package`, and both C generators flatten a module path into the name, so a
keyword there is a segment of a longer identifier that no compiler reads as a
keyword. A type declared at the outermost scope, an enumerator and a bitmask
flag each stand alone in the generated source.

### fixed<P,S>

| case | construct | Rust probe |
|---|---|---|
| [`fixed-basic`](fixed-basic/) | `fixed<9,3>` | yes |
| [`fixed-basic-unannotated`](fixed-basic-unannotated/) | `fixed<9,3>`, unannotated | yes |
| [`fixed-scale-equals-digits`](fixed-scale-equals-digits/) | `fixed` with scale equal to precision | yes |
| [`fixed-scale-equals-digits-unannotated`](fixed-scale-equals-digits-unannotated/) | `fixed` with scale equal to precision, unannotated | yes |
| [`fixed-scales`](fixed-scales/) | `fixed` at several precisions and scales | yes |
| [`fixed-scales-unannotated`](fixed-scales-unannotated/) | `fixed` at several precisions and scales, unannotated | yes |

### Recursion

| case | construct | Rust probe |
|---|---|---|
| [`recursive-array`](recursive-array/) | a self-reference through an array — refused by name, IDL 4.2 §7.4.1.4.4.4.4 | refused |
| [`recursive-mutual`](recursive-mutual/) | two types that reach each other through sequences | yes |
| [`recursive-mutual-optional`](recursive-mutual-optional/) | two types on one cycle, one member `@optional` and one not | yes |
| [`recursive-optional`](recursive-optional/) | the same self-reference held only by `@optional`, with no `@external` | yes |
| [`recursive-optional-external`](recursive-optional-external/) | a self-reference held only by `@optional @external` | yes |
| [`recursive-self-sequence`](recursive-self-sequence/) | a type that names itself through a sequence | yes |
| [`recursive-through-map`](recursive-through-map/) | a type that names itself through a `map` value | yes |
| [`recursive-typedef-sequence`](recursive-typedef-sequence/) | the same cycle written through a typedef | yes |
| [`recursive-union-branch`](recursive-union-branch/) | a cycle that closes through a union branch's sequence | yes |

### Combined shapes

| case | construct | Rust probe |
|---|---|---|
| [`combined-everything`](combined-everything/) | one aggregate holding most of the corpus | yes |
| [`combined-everything-unannotated`](combined-everything-unannotated/) | one aggregate holding most of the corpus, unannotated | yes |
| [`combined-keyed-topic`](combined-keyed-topic/) | a keyed topic type | yes |
| [`combined-keyed-topic-unannotated`](combined-keyed-topic-unannotated/) | a keyed topic type, unannotated | yes |

### Evolution — two IDL files, the older writing and the newer reading

| case | construct | Rust probe |
|---|---|---|
| [`evolution-mutable-tail`](evolution-mutable-tail/) | a `@mutable` member the writer's type did not have | yes |
| [`evolution-mutable-middle`](evolution-mutable-middle/) | the same, in the middle and of variable length | yes |
| [`evolution-mutable-defaults`](evolution-mutable-defaults/) | one member per row of Table 9 | yes |
| [`evolution-mutable-optional`](evolution-mutable-optional/) | the same, and the receiver's member is `@optional` | yes |
| [`evolution-mutable-mustunderstand`](evolution-mutable-mustunderstand/) | the same, and it is `@must_understand` | yes |
| [`evolution-mutable-key`](evolution-mutable-key/) | the same, and it is a `@key` | yes |
| [`evolution-mutable-default-annotation`](evolution-mutable-default-annotation/) | the same, with an explicit `@default` | yes |
| [`evolution-mutable-union`](evolution-mutable-union/) | the same, of union type — Table 9's recursive case | yes |
| [`evolution-mutable-map`](evolution-mutable-map/) | the same, of `map` type | yes |
| [`evolution-appendable-tail`](evolution-appendable-tail/) | an `@appendable` tail the writer's type did not have | yes |
| [`evolution-appendable-nested`](evolution-appendable-nested/) | the same, nested in a `@final` type | yes |
| [`evolution-union-unselected`](evolution-union-unselected/) | a `@mutable` union whose newer version gives an existing discriminator value a case | yes |
| [`evolution-final-tail`](evolution-final-tail/) | a `@final` pair whose member sets differ, which §7.2.4.4.8 forbids | yes |
| [`evolution-mutable-reordered`](evolution-mutable-reordered/) | a `@mutable` member list in an order the reader does not declare — the same ids, written out, in a different declaration order | yes |
| [`evolution-final-to-appendable`](evolution-final-to-appendable/) | a `@final` writer and an `@appendable` reader over the same member set — the #41, #42 and #27 failure | yes |

347 cases. 344 serialise under `--encoding xcdr2 --endian le` and under each of the other three combinations, 3 do not; every row above was produced by running the case, not by reading the source.
