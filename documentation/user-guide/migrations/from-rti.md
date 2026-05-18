# Migration from RTI Connext DDS (legacy notes)

These migration notes are early drafts. See
[`07-migration/from-rti-connext.md`](../../07-migration/from-rti-connext.md)
for the current migration playbook.

This document captures what the ZeroDDS IDL parser supports for
RTI-specific input today, what it does not, and what typical
migration patterns look like.

## What works

### `keylist` directive

```idl
struct Sensor {
    long sensor_id;
    double value;
};
keylist Sensor (sensor_id);
```

Activate the RTI delta in the parser:

```rust
use zerodds_idl::grammar::deltas::RTI_CONNEXT;
use zerodds_idl::parser::parse_with_deltas;

parse_with_deltas(src, &cfg, &[&RTI_CONNEXT])?;
```

### `@key` annotation as the modern equivalent

Standard OMG annotation, works without any delta:

```idl
@topic
struct Sensor {
    @key long sensor_id;
    double value;
};
```

Migration tip: prefer `@key` over `keylist` — OMG-conformant and
vendor-portable.

### Vendor-specific annotations

Annotation names are `<scoped_name>`s — RTI-specific annotations
parse without modification:

```idl
@rti::transfer_mode("RELIABLE")
@rti::language_binding("CPP11")
struct Foo { @key long id; };
```

The parser accepts these with the base grammar; semantic
validation lives in the RTI vendor adapter.

### `#include` / `#define` / `#ifdef`

C-style preprocessor with `#include "Foo.idl"`,
`#include <Sys.idl>`, `#define MACRO value`,
`#ifdef`/`#ifndef`/`#else`/`#endif`. `#pragma` is stripped.

```rust
use zerodds_idl::preprocessor::{Preprocessor, MemoryResolver};
let pp = Preprocessor::new(MemoryResolver::new());
let processed = pp.process("main.idl", src)?;
```

## Migration workflow

1. Parse IDL and collect diagnostics:
   ```bash
   zerodds-idlc --parse-only --rti your_topics.idl
   ```
2. Migrate legacy `#pragma keylist` to a `keylist` clause or
   `@key` annotation:
   - `#pragma keylist X y` → `keylist X (y);` or a `@key` member.
3. Review `@rti::*` annotations — they remain functional via the
   catch-all `Definition::VendorExtension`.
4. `valuetype` full form (custom / init / factory) is reachable
   through the CORBA-coexistence path.

## Sales point

The RTI Connext delta in
`crates/idl/src/grammar/deltas/rti_connext.rs` is **100 LOC and
zero hacks in the base grammar**. Vendor-specific extensions are
additive patches — the OMG-IDL 4.2 base stays the single source
of truth.

Consequences for migration projects:

- No grammar fork to maintain.
- OMG conformance is preserved.
- Multi-vendor code bases (e.g. RTI + Cyclone) parse with
  composed deltas without conflict.

## References

- [`crates/idl/README.md`](../../../crates/idl/README.md) —
  pipeline overview
- [`crates/idl/src/grammar/deltas/rti_connext.rs`](../../../crates/idl/src/grammar/deltas/rti_connext.rs)
  — delta source
- [RTI Connext DDS Documentation](https://community.rti.com/static/documentation/connext-dds/)
