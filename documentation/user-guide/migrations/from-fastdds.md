# Migration from eProsima Fast-DDS (legacy notes)

These migration notes are early drafts. See
[`07-migration/from-fastdds.md`](../../07-migration/from-fastdds.md)
for the current migration playbook.

## Quick orientation

Fast-DDS uses predominantly **standard OMG-IDL 4.2** with
standard annotations. The ZeroDDS base grammar parses Fast-DDS
IDL files **without a delta** (see `crates/idl/tests/parse_vendors.rs`).

What works without changes:

- `@key`, `@final`, `@mutable`, `@appendable`
- `@id`, `@optional`
- `@topic` (Fast-DDS topic marker)
- Standard constructed types
- `sequence<T,N>`, `string<N>`

Example fixture (`crates/idl/tests/fixtures/fastdds/`):

```idl
@final
struct HelloWorld {
    @key unsigned long index;
    string<256> message;
};
```

Run the parse-roundtrip:

```bash
cargo test -p zerodds-idl --test parse_vendors fastdds
```

## References

- [`crates/idl/tests/fixtures/fastdds/`](../../../crates/idl/tests/fixtures/fastdds/)
- [eProsima Fast-DDS](https://www.eprosima.com/index.php/products-all/eprosima-fast-dds)
