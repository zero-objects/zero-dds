# Changelog

Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionierung folgt [Semantic Versioning](https://semver.org/lang/de/).

## [1.0.0-rc.1] — 2026-05-07

Initiale Release-Materialisierung der `zerodds-corba-ior`-Crate.

### Spec-Referenzen

- **OMG CORBA 3.3 Part 2**: §13.6 (Object References),
  §13.6.2 (IOR-Struct), §13.6.7.1 (Profile-Tags),
  §13.6.7.3 (TaggedComponents), §13.6.10 (Stringified-IOR +
  corbaloc/corbaname URL-Schemes), §15.7.2 (IIOP-ProfileBody-Inhalt
  des `TAG_INTERNET_IOP`-Profils), §10 (CSIv2 CompoundSecMechList).

### Public-API

- `ior::Ior` — IOR-Struct (`type_id` + `Vec<TaggedProfile>`).
- `tagged_profile::TaggedProfile` — Profile-Variants
  (`MultipleComponents`, `InternetIop` mit `IiopProfileBody` aus
  `corba-iiop`, plus opaque-Variante).
- `profile_tags::ProfileId` — `InternetIop`, `MultipleComponents`,
  `ScCorbalocImr` und Vendor-Tags.
- `components::{TaggedComponent, StructuredComponent,
  AlternateIiopAddress, CodeSetComponent, CodeSetComponentInfo,
  OrbType, Ssl, StreamFormatVersion, TlsSecTrans}` — alle 32
  Standard-Component-Tags inkl. strukturierter Decoder fuer:
  ORB_TYPE / CODE_SETS / ALTERNATE_IIOP_ADDRESS / SSL_SEC_TRANS /
  TLS_SEC_TRANS / RMI_CUSTOM_MAX_STREAM_FORMAT / JAVA_CODEBASE /
  CSI_SEC_MECH_LIST.
- `component_tags::ComponentId` — `unsigned long`-Tags.
- `stringified::{STRINGIFIED_IOR_PREFIX, from_stringified,
  to_stringified}` — `IOR:`-Prefix + Hex-Encoding einer CDR-
  Encapsulation (Spec §13.6.10).
- `url::{CorbalocAddress, CorbanameAddress, parse_corbaloc,
  parse_corbaname}` — URL-Parser fuer `corbaloc:` und `corbaname:`-
  Schemes.
- `error::{IorError, IorResult}` — Error-Surface.

### Implementierung

`#![cfg_attr(not(feature = "std"), no_std)]` mit `extern crate alloc`;
`#![forbid(unsafe_code)]`.

CDR-Encapsulation pro IOR-Roundtrip; Endianness-Marker im ersten
Byte gemaess CDR-1-Spec. `to_stringified`/`from_stringified` runden
strikt durch Hex-Encoding.

`StructuredComponent::CsiSecMechList` ist die typisierte Form des
`TAG_CSI_SEC_MECH_LIST`-Tags (CSIv2 §10), Cross-Crate-Wire-up via
`zerodds-corba-csiv2::CompoundSecMechList`. F-CORBA-CSIV2-NOT-WIRED
ist damit external resolved.

URL-Parser akzeptieren multi-Address-Syntax (`addr,addr,...`) gemaess
§13.6.10.

### Architektur

- **Layer:** 8 (CORBA-Stack, Tier-C).
- **Dependencies (in):** `zerodds-cdr`, `zerodds-corba-csiv2`,
  `zerodds-corba-iiop`.
- **Dependents (out):** `zerodds-corba-cosnaming` (NameBinding-Object-
  Refs), `zerodds-corba-dds-bridge` (object_key-Mapping).
- **Feature-Flags:** `std` (default), `alloc` (via std).

### Stabilitaet

- Public-API: RC1-stabil.
- Wire-Format durch OMG fixiert.
- `CsiSecMechList`-Variante neu in rc1 (Cross-Crate-Resolution
  F-CORBA-CSIV2-NOT-WIRED).
