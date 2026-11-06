# `zerodds-corba-dds-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-corba-dds-bridge/badge.svg)](https://docs.rs/zerodds-corba-dds-bridge)

The DDS↔CORBA bridge daemon `zerodds-corba-bridged`
(`docs/specs/zerodds-corba-bridge-1.0.md`): a GIOP request to a mapped
object key and operation becomes a sample on the mapping's request topic,
and the sample on its reply topic becomes the GIOP reply (§4.3); a sample on
a notify topic becomes a oneway GIOP request to each target (§4.4). IIOP and
SSLIOP listeners, CSIv2, the NameService, LocateRequest.
`forbid(unsafe_code)`. Safety classification: **STANDARD**.

## Spec mapping

| Spec | Section |
|------|-----------|
| `zerodds-corba-bridge-1.0` | §3 configuration, §4 GIOP/IIOP, §6 QoS, §7 security, §8 operations, §9 lifecycle |
| OMG CORBA 3.3 Part 2 | §15 GIOP, §13.6 IOR |

## What's included

- **`bridge_daemon`** — the configuration (§3), the IIOP and SSLIOP
  listeners, the request/reply and notify mappings, LocateRequest (§4.7)
  and the DDS side.
- **`BridgeMapping`**, **`BridgeRoute`**, **`OperationMapping`**,
  **`TopicQosRef`**, **`Direction`** — mappings as code, as the dds-chat
  tutorial and the dds-warehouse demo write them; the daemon reads its
  mappings from its configuration.
- **`bridge_security`**, **`csiv2_wire`** — TLS for SSLIOP, CSIv2 (§7).
- **`naming_client`** — binds the IORs in a NameService.
- **`daemon_runtime`** — metrics, health, shutdown (§8, §9).

## Example

```rust
use zerodds_corba_dds_bridge::Direction;
assert_ne!(Direction::CorbaToDds, Direction::DdsToCorba);
```

## Tests

```bash
cargo test -p zerodds-corba-dds-bridge
```

## Interop measurements: JacORB 3.9 / omniORB / TAO

Every ZeroDDS-side command/config key below is this crate's own,
exercised by `tests/naming_service_e2e.rs`/`tests/csiv2_e2e.rs`/
`tests/security_e2e.rs` — 100% confidence. The foreign-ORB commands are
sourced from JacORB's own GitHub repo, omniORB's manpages and TAO's
docs (linked per subsection below); none were run against a real
JacORB/omniORB/TAO install from this repo, so treat them as a starting
point against your own install, not as a measured result. Where a
vendor detail could not be confirmed this round, it says so instead of
guessing.

### A. A foreign NameService as `corba.ior.naming_service`'s rebind target

The bridge's naming client resolves any `corbaname:`/`corbaloc:` URL
and rebinds every receiving mapping's servant into it (round 2 item
3). Pointing it at a foreign vendor's NameService instead of
`zerodds-naming-svc` is the cross-vendor check.

**omniORB — `omniNames`**

```bash
omniNames -start 2809 -logdir /tmp/omninames-log
```

```yaml
corba:
  ior:
    naming_service: "corbaloc::<host>:2809/NameService"
    publish_to_naming: true
```

**TAO — `tao_cosnaming`**

```bash
tao_cosnaming -ORBListenEndpoints iiop://<host>:12345 -m 0
```

```yaml
corba:
  ior:
    naming_service: "corbaloc:iiop:<host>:12345/NameService"
    publish_to_naming: true
```

**JacORB — `ns` (`/opt/jacorb/bin/ns`)**

```bash
/opt/jacorb/bin/ns -DOAPort=2809 \
  -Djacorb.naming.ior_filename=/tmp/jacorb-ns.ior
```

```yaml
corba:
  ior:
    naming_service: "corbaloc::<host>:2809/NameService"
    publish_to_naming: true
```

JacORB's naming server answers `corbaloc::host:port/NameService`
directly — no IOR-file exchange needed bridge-side.

Sources: [JacORB `Nameservice.tex`](https://github.com/JacORB/JacORB/blob/master/doc/ProgrammingGuide/Naming/Nameservice.tex),
[Wikibooks: ACE+TAO — Find a service on a naming server](https://en.wikibooks.org/wiki/ACE%2BTAO_Opensource_Programming_Notes/Find_a_service_on_a_naming_server),
[omniORB `nameclt(1)`](https://manpages.debian.org/experimental/omniorb/nameclt.1.en.html).

### B. A JacORB client, CSIv2 GSSUP over SSLIOP, against the bridge

Bridge-side config (round 2 items 1/4's ContextError codes and mutual
TLS apply automatically; this only needs CSIv2 + SSLIOP turned on):

```yaml
corba:
  iiop:
    bind: "0.0.0.0:6833"
  ssliop:
    enabled: true
    bind: "0.0.0.0:6834"
    cert_file: "/etc/zerodds/bridge.crt"
    key_file: "/etc/zerodds/bridge.key"
    # ca_file + require_client_cert: true only if the JacORB client
    # also presents a client certificate (SSLIOP
    # EstablishTrustInClient) -- GSSUP authenticates the *user*, this
    # authenticates the *channel*; the two are independent.
  csiv2:
    enabled: true
    gssup:
      password_file: "/etc/zerodds/gssup.passwd"
acl:
  default_deny: true
  rules:
    - subject: "gssup:alice"
      allow_invoke: ["*"]
mappings:
  - repo_id: "IDL:zerodds_test/Echo:1.0"
    operation: call
```

`gssup.passwd` holds one `username:salt_hex:iterations:hash_hex` line
per user ([`gssup_store` module doc](../corba-csiv2/src/gssup_store.rs)).
There is no CLI for it yet — `zerodds_corba_csiv2::gssup_store::format_entry`
is a library call; `tests/csiv2_e2e.rs` builds one with
`format_entry("alice", "secret", b"e2e-salt", 1000)` from a throwaway
Rust binary or `cargo test`, the same way you would for a real user.

JacORB-side `jacorb.properties` (property names confirmed from JacORB
3.9's own `etc/jacorb_properties.template` and `test/sas/TLS.properties`
on GitHub, not from memory):

```properties
jacorb.security.support_ssl=on
jacorb.security.ssl.client.supported_options=40
jacorb.security.ssl.client.required_options=40
jacorb.security.ssl.corbaloc_ssliop.supported_options=40
jacorb.security.ssl.corbaloc_ssliop.required_options=40

jacorb.security.sas.contextClass=org.jacorb.security.sas.GssUpContext
org.omg.PortableInterceptor.ORBInitializerClass.SAS=org.jacorb.security.sas.SASInitializer
org.omg.PortableInterceptor.ORBInitializerClass.GSSUPProvider=org.jacorb.security.sas.GSSUPProviderInitializer

ORBInitRef.NameService=corbaloc:ssliop:1.2@<bridge-host>:6834/NameService
```

`0x40` = `EstablishTrustInClient` only (the client trusts the target's
certificate, presents none of its own back) — matches
`corba.ssliop.require_client_cert` left at its default (`false`)
above. For mutual TLS (round 2 item 4's
`ClientAuth::Required`), also set `corba.ssliop.ca_file` +
`require_client_cert: true` bridge-side and give the JacORB client its
own keystore; JacORB's `doc/ProgrammingGuide/SSL/ssl.tex` covers the
keystore properties, not reproduced here because the exact keystore
property names were not independently confirmed this round.

**GSSUP credentials are not a `jacorb.properties` key.** Confirmed from
JacORB 3.9's own [`GssUpContext` Javadoc](https://jacorb.github.io/JacORB/org/jacorb/security/sas/GssUpContext.html):
a client supplies them programmatically —
`GssUpContext.setUsernamePassword("alice", "secret")` — called once
before the CSIv2-secured invocation. A config-file-only client has no
hook for this; measuring this path needs a small JacORB Java client
(or a JacORB demo that already calls it), not a properties file alone.

Sources: [`etc/jacorb_properties.template`](https://github.com/JacORB/JacORB/blob/master/etc/jacorb_properties.template),
[`test/sas/TLS.properties`](https://github.com/stuartwdouglas/jacorb/blob/master/test/sas/TLS.properties),
[`GssUpContext` Javadoc](https://jacorb.github.io/JacORB/org/jacorb/security/sas/GssUpContext.html),
[Micro Focus: SSL supported/required properties in jacorb.properties](https://community.microfocus.com/cobol/openfusion/w/wikiid-188/33938/using-the-ssl-supported-and-required-properties-in-the-jacorb-properties-file)
(association-option hex values).

## See also

- [`zerodds-corba-giop`](../corba-giop/README.md) — GIOP request.
- [`zerodds-corba-ior`](../corba-ior/README.md) — IOR / object_key.
- [Architecture](../../docs/architecture/02_architecture.md)
