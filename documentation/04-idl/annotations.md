# IDL annotations

Annotations refine the codegen and wire-format. ZeroDDS implements
the full XTypes 1.3 set plus the customary OMG built-ins.

## Per-type

| Annotation | Purpose | Example |
|---|---|---|
| `@final` | Tight wire packing, no extensibility | `@final struct …` |
| `@appendable` | Length-prefixed; new fields may be appended | `@appendable struct …` |
| `@mutable` | Per-member ID/length/value; full evolution | `@mutable struct …` |
| `@nested` | Type is only used as nested member, not top-level | `@nested struct Pose` |
| `@bit_bound(N)` | Underlying integer width for `bitset`/`bitmask` | `@bit_bound(8) bitset Flags` |
| `@autoid(SEQUENTIAL\|HASH)` | How member IDs are auto-assigned | `@autoid(HASH) @mutable struct …` |

## Per-member

| Annotation | Purpose | Example |
|---|---|---|
| `@key` | Field contributes to instance KeyHash | `@key string<32> id;` |
| `@id(N)` | Explicit Member-ID for `@mutable` | `@id(0x42) double y;` |
| `@hashid` | Auto-derive Member-ID from member name | `@hashid double y;` |
| `@hashid("seed")` | As above, with a non-default hash seed | `@hashid("ROS") double y;` |
| `@optional` | Field may be absent on the wire | `@optional double battery_pct;` |
| `@must_understand` | Reader must understand this field or reject sample | `@must_understand long version;` |
| `@external` | Field is heap-stored (boxed) — for recursive types | `@external Self next;` |
| `@try_construct(USE_DEFAULT\|TRIM\|DISCARD)` | What to do if value violates constraint | `@try_construct(TRIM) string<10> name;` |
| `@unit("m/s")` | Unit annotation, propagated to documentation | `@unit("m") double x;` |
| `@min(0.0)` / `@max(100.0)` | Range constraints | `@min(0.0) double pct;` |
| `@default(VAL)` | Default value if absent (with `@optional`) | `@default(0.0) @optional double bias;` |
| `@verbatim("rust", "…")` | Inject language-specific code | see below |

## `@verbatim` example

Per XTypes 1.3 §7.2.2.4.8, `@verbatim` injects raw code into the
generated output. ZeroDDS supports six placement kinds:

```idl
struct Demo {
    @verbatim(language="rust", placement="prefix",
              text="impl Demo { pub fn unit_norm(&self) -> f64 { /* … */ 0.0 } }")
    double x;
    double y;
};
```

| Placement | Where |
|---|---|
| `prefix` | Before the type definition |
| `inside` | Inside the type body |
| `suffix` | After the type definition |
| `import` | At the top of the file |
| `header` | Header comment area |
| `footer` | At the bottom of the file |

`language` matches the codegen target: `rust`, `cpp`, `java`,
`csharp`, `python`, `typescript`. Unmatched languages skip the
verbatim block silently.

## Annotation-name conventions

OMG specs are inconsistent. ZeroDDS accepts:

- `@key` (XTypes) and `@KeyHash` (older RTI dialect) — both work.
- `@id(N)` and `@ID(N)` — case-insensitive.
- `@final` / `@Final` — case-insensitive.

The IDL parser folds known annotation names to their canonical
form before AST building.

## Custom annotations

Custom annotations with arbitrary parameters parse fine but are
ignored by the codegen unless you add a backend hook. See
`crates/idl-cpp/src/annotations.rs` for an example.

## Compatibility cheat-sheet

| Old style (some vendors) | XTypes 1.3 canonical |
|---|---|
| `//@Key` comment | `@key` annotation |
| `pragma keylist X y;` | `@key` per member |
| `@TopicAttributes(stability="immutable")` | `@final` |
| `@MemberAttribute(id=N)` | `@id(N)` |

`zerodds-idlc` accepts both styles for backward compatibility.

## Reading further

- OMG XTypes 1.3 §7.3 — annotations and TypeObject.
- `crates/idl/src/builder/annotations.rs` — implementation.
