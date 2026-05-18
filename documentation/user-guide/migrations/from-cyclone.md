# Migration from Eclipse Cyclone DDS (legacy notes)

These migration notes are early drafts. See
[`07-migration/from-cyclonedds.md`](../../07-migration/from-cyclonedds.md)
for the current migration playbook.

## Quick orientation

Cyclone DDS uses **standard OMG-IDL 4.2** without vendor grammar
extensions. The ZeroDDS base grammar (`IDL_42`) parses Cyclone IDL
files **without a delta** (see `crates/idl/tests/parse_vendors.rs`).

What works without changes:

- `@key`, `@final`, `@appendable`, `@mutable`, `@nested`
- `@id`, `@optional`, `@default_literal`
- `@unit`, `@bit_bound`, `@verbatim`
- Modules, structs (with inheritance), unions, enums, bitsets, bitmasks
- `map<K,V>`, bounded / unbounded sequence + string

Example fixture (`crates/idl/tests/fixtures/cyclonedds/`):

```idl
@final
struct ThroughputType {
    @key octet count;
    sequence<octet> payload;
};
```

Run the parse-roundtrip:

```bash
cargo test -p zerodds-idl --test parse_vendors cyclone
```

## References

- [`crates/idl/tests/fixtures/cyclonedds/`](../../../crates/idl/tests/fixtures/cyclonedds/)
- [Eclipse Cyclone DDS](https://cyclonedds.io/)
