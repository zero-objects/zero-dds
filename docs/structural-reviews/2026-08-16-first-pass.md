# First structural review — `inherit-two-levels`, `optional-nested-struct`

Columns: cpp, java, csharp, ts, python · foreign: hdds
Tree: `main` at the time of the review, 2026-08-16
Material: `tools/struct-compare/collect.sh`, questions from
`docs/structural-review.md`

The first run of this review. Two cases, chosen because they are the two
axes where a generated API can diverge without any octet moving:
inheritance and optionality.

## Findings

### 1. Every column flattens inheritance; the one foreign implementation nests it

Kind: **ours**, and defensible — but undocumented until now
Columns: all five, against HDDS

`struct Base { long a; }`, `struct Mid : Base { long b; }`,
`struct Leaf : Mid { long c; }`:

| column | shape |
|---|---|
| C++ | `struct Leaf { std::int32_t a; b; c; }` |
| Java | `class Leaf { int a; b; c; }` |
| C# | `class Leaf { int A; B; C; }` with `[IdlName]` on each |
| TypeScript | `interface Leaf { a; b; c: number }` |
| Python | `class Leaf: a: _Int32; b; c` |
| HDDS | `struct Leaf { base: Mid, c }` — **nested** |

XTypes 1.3 §7.4.1.1.5.1 fixes the wire order — "as if all of the members
had been defined, in the same order, in the most-derived type" — and
says nothing about the API. Both shapes are conformant. Flattening is
the better choice for a generated API and all five of our columns make
it consistently, which is the part worth having written down: the
consistency was never stated anywhere and could have drifted per column
without any test noticing.

Action: none for the shape. Recorded here, and it is what
`docs/edge-case-map.md` should cite when it explains why the HDDS probe
needed a nested literal.

### 2. Optionality is four different programs, and each one is right

Kind: **language-forced**
Columns: all five

`@optional Inner i;`:

| column | shape |
|---|---|
| C++ | `std::optional<::m::Inner> i;` |
| Java | `ZdTypes.m.Inner i;` — null when absent |
| C# | nullable reference, `[IdlName("i")]` |
| TypeScript | `i?: m.Inner` — absent rather than present-and-null |
| Python | `typing.Optional[m.Inner] = None` |

Java and C# have no `optional` type for a reference and use null, which
is the language's own idiom; TypeScript distinguishes absent from null
and takes absent, which is the one that matches XTypes §7.4.1.2.3
exactly — absence *is* the value. Nothing to change.

What is worth noting: **every column carries the same clause citation in
its doc comment**, generated from one place. A user reading the Java
field and a user reading the Python field are told the same thing about
the wire, in their own language's comment syntax. That is the property
this layer was rebuilt for, and it holds.

### 3. Two columns discard the IDL spelling and three keep it

Kind: **ours**
Columns: C and C++ against C#, Python, TypeScript

C# writes `[IdlName("a")]` on every field; Python keeps the IDL name in
its annotations; TypeScript's members keep their IDL spelling outright.
C and C++ prefix a colliding identifier and record nothing.

Nothing about C forces that. And it has a consequence beyond
readability: XTypes 1.3 §7.3.1.2.1.1 computes a hashed member id from
the **IDL name**, so a column that cannot recover the IDL spelling from
its generated type cannot compute a member id for a member whose name
had to move.

Action: an open item. The C and C++ columns need somewhere to carry the
IDL spelling — a comment is not enough if anything is to read it back.

## Summary

Three findings: one language-forced, two ours. Of the two, finding 1 is
a decision that turned out consistent and is now written down, and
finding 3 is a real gap with a wire consequence.

The count to watch across reviews is **ours**, and the first pass sets
it at two.
