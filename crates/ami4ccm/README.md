# `zerodds-ami4ccm`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-ami4ccm/badge.svg)](https://docs.rs/zerodds-ami4ccm)

OMG AMI4CCM 1.1 (`formal/2015-08-03`) — Asynchronous Method Invocation
for the CORBA Component Model. Implied-IDL transformation,
ExceptionHolder model, pragma parsing, connector/deployment models.
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| OMG AMI4CCM 1.1 | §7.3 (implied IDL for the AMI4CCM interface) |
| OMG AMI4CCM 1.1 | §7.4 (ExceptionHolder data model) |
| OMG AMI4CCM 1.1 | §7.5 (implied IDL for the ReplyHandler) |
| OMG AMI4CCM 1.1 | §7.6 (connector model) |
| OMG AMI4CCM 1.1 | §7.7 (pragmas `ami4ccm interface` + `ami4ccm receptacle`) |
| OMG AMI4CCM 1.1 | §7.8 (deployment plan fragment) |

## What is included

- **`pragma`** — `parse_pragma(line)` + `Ami4CcmPragma::{Interface,
  Receptacle}` with a whitespace-tolerant quoted-name parser form.
- **`transform`** — `transform_interface` + `transform_interface_in_context`
  derive, from a `zerodds_idl::ast::InterfaceDef`, the two
  derived local interfaces `AMI4CCM_<Iface>` +
  `AMI4CCM_<Iface>ReplyHandler`.
- **`exception_holder`** — `ExceptionHolder` + `UserExceptionBase` as the
  data model for spec §7.4.1 exception delivery.
- **`scope_resolver`** — `populate_from_specification` /
  `context_from_specification` for cross-module type resolution.
- **`connector`** + **`deployment`** + **`multiplex`** — connector,
  plan-fragment, multi-receptacle models (spec §7.6 + §7.8).

## What is not covered

- **AMI4CCM connector runtime** (spec §7.6 hosting portion) — the
  connector fragment code is deployed via D&C into a CCM container;
  ZeroDDS has no CCM-container hosting of its own (see
  `crates/corba-ccm/` for the IDL wrapper layer).
- **CCM pragma pre-processor integration** — pragma parsing is realized
  as a standalone function; integration into the IDL
  preprocessor can be built on top of it once CCM is a
  top-level sprint.

## Example

```rust
use zerodds_ami4ccm::{Ami4CcmPragma, parse_pragma};

let p = parse_pragma("#pragma ami4ccm interface \"Stock::StockManager\"").unwrap();
assert_eq!(
    p,
    Ami4CcmPragma::Interface {
        name: "Stock::StockManager".into(),
    }
);
```

## Tests

```bash
cargo test -p zerodds-ami4ccm
```

## See also

- [Architecture](../../docs/architecture/02_architecture.md)
- [Components](../../documentation/02-architecture/components.md)
- [Spec-Coverage Audit](../../docs/spec-coverage/omg-ami4ccm-1.1.md)
