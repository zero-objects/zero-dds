# `zerodds-ami4ccm`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-ami4ccm/badge.svg)](https://docs.rs/zerodds-ami4ccm)

OMG AMI4CCM 1.1 (`formal/2015-08-03`) — Asynchronous Method Invocation
fuer das CORBA Component Model. Implied-IDL-Transformation,
ExceptionHolder-Modell, Pragma-Parsing, Connector-/Deployment-Modelle.
`no_std + alloc`, `forbid(unsafe_code)`. Safety classification:
**STANDARD**.

## Spec-Mapping

| Spec | Abschnitt |
|------|-----------|
| OMG AMI4CCM 1.1 | §7.3 (Implied-IDL fuer AMI4CCM-Interface) |
| OMG AMI4CCM 1.1 | §7.4 (ExceptionHolder-Datenmodell) |
| OMG AMI4CCM 1.1 | §7.5 (Implied-IDL fuer ReplyHandler) |
| OMG AMI4CCM 1.1 | §7.6 (Connector-Modell) |
| OMG AMI4CCM 1.1 | §7.7 (Pragmas `ami4ccm interface` + `ami4ccm receptacle`) |
| OMG AMI4CCM 1.1 | §7.8 (Deployment-Plan-Fragment) |

## Was ist drin

- **`pragma`** — `parse_pragma(line)` + `Ami4CcmPragma::{Interface,
  Receptacle}` mit Whitespace-toleranter Quoted-Name-Parser-Form.
- **`transform`** — `transform_interface` + `transform_interface_in_context`
  erzeugen aus einem `zerodds_idl::ast::InterfaceDef` die zwei
  abgeleiteten Local-Interfaces `AMI4CCM_<Iface>` +
  `AMI4CCM_<Iface>ReplyHandler`.
- **`exception_holder`** — `ExceptionHolder` + `UserExceptionBase` als
  Datenmodell fuer Spec §7.4.1 Exception-Lieferung.
- **`scope_resolver`** — `populate_from_specification` /
  `context_from_specification` fuer Cross-Module-Type-Resolution.
- **`connector`** + **`deployment`** + **`multiplex`** — Connector-,
  Plan-Fragment-, Multi-Receptacle-Modelle (Spec §7.6 + §7.8).

## Was nicht abgedeckt ist

- **AMI4CCM-Connector-Runtime** (Spec §7.6 Hosting-Anteil) — der
  Connector-Fragment-Code wird via D&C in einen CCM-Container
  deployed; ZeroDDS hat kein eigenes CCM-Container-Hosting (siehe
  `crates/corba-ccm/` fuer den IDL-Wrapper-Layer).
- **CCM-Pragma-Pre-Processor-Integration** — Pragma-Parsing ist als
  eigenstaendige Funktion realisiert; Integration in den IDL-
  Preprocessor laesst sich darauf aufbauen, sobald CCM ein
  Top-Level-Sprint ist.

## Beispiel

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
