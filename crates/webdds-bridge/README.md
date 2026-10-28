# `zerodds-webdds-bridge`

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0)
[![docs.rs](https://docs.rs/zerodds-webdds-bridge/badge.svg)](https://docs.rs/zerodds-webdds-bridge)

The OMG Web-Enabled DDS 1.0 (`formal/2014-12-01`) gateway daemon,
`zerodds-webdds-bridged`. Serves the REST PSM (§8.3, `crates/web`) and
the SIMPLE-WSDL-SOAP PSM (§8.4, `crates/zerodds-soap`) from one process
on one HTTP port — two platform mappings of the same WebDDS object
model (Spec §1.5), backed by `zerodds-dcps`'s public typed API.

`crates/web` and `crates/zerodds-soap` are pure `no_std + alloc`
libraries: routing, status-code mapping, XML element-tag registries,
SOAP envelope/fault/WSDL codecs. Neither implements an HTTP server, a
`DdsBackend`, or a body codec — `crates/web`'s own crate doc says as
much. This crate is that caller layer.

## What's inside

- **`gateway::Gateway`** — `impl zerodds_web::bridge::DdsBackend`,
  backed by `zerodds-dcps`'s typed API (`DomainParticipant`, `Topic`,
  `Publisher`/`Subscriber`, `DataWriter`/`DataReader`, `WaitSet`).
  Covers every §7.4 entity operation: create/update/delete/list for
  Application/Participant/Topic/Publisher/Subscriber/DataWriter/
  DataReader (update = `set_qos` on the live `zerodds-dcps` entity,
  `zerodds_dcps::Entity`), write/read for samples, create/update/wait/
  delete/list for WaitSets (§7.4.9's long-poll `WaitSet::get`).
- **`qos_update`** — turns a `PUT` body's inline `<qos>` container
  (`zerodds_xml::EntityQos`), or a `<qos_profile>name</qos_profile>`
  reference resolved through the daemon's own `QosLibrary`/`QosProfile`
  metadata store (override-wins merge when both are present), into the
  target entity's concrete `zerodds_dcps::qos` value, merged onto that
  entity's own current QoS.
- **`contained_entities`** — parses the nested `<publishers>`/
  `<subscribers>`/`<data_writers>`/`<data_readers>` a
  `create_participant`/`update_participant`/`create_publisher`/
  `update_publisher`/`create_subscriber`/`update_subscriber` body may
  carry (Spec §7.4.3.1/§7.4.3.2/§7.4.4.7/§7.4.4.8/§7.4.4.11/§7.4.4.12);
  `crate::gateway` reconciles the live DCPS entities against the
  parsed set by name (add, re-apply QoS, or remove).
- **`types_registry::TypeRegistry`** — resolves a topic's wire type
  from a config-named `.idl` file, or from a peer's TypeLookup
  discovery, into a `SerializedTypeSupport` plus a `DynamicType`
  reflective view.
- **`dynxml`** — converts a `DynamicData` sample to/from the DDS-WEB
  §8.1 XML representation, driven by the type's own descriptor.
- **`metadata::MetadataStore`** — §7.3.1 `Root` metadata
  (QosLibrary/QosProfile/Type CRUD) — not a DCPS entity, so it is not
  part of `DdsBackend`.
- **`wsdl_bridge`** — builds the SOAP WSDL from the fixed DDS-WEB
  object-model operations plus every config-named IDL `interface`'s
  own operations (`zerodds_lang_rules::adapter::interface`).
- **`rest_service` / `soap_service`** — request dispatch.
- **`http1`** — a hand-rolled HTTP/1.1 engine (request line + headers,
  `Content-Length` and chunked request bodies with a size cap,
  keep-alive). No framework dependency.
- **`daemon`** — config/CLI/security/server, the same shape as every
  other `zerodds-*-bridged` daemon.

## Layer position

Layer 5 — bridges/gateways. The caller layer for `zerodds-web` +
`zerodds-zerodds-soap`.

## Quickstart

```bash
zerodds-webdds-bridged --config /etc/zerodds/webdds-bridged.yaml
```

or, for a quick local run without a config file:

```bash
zerodds-webdds-bridged --listen 127.0.0.1:8080 --domain 0 \
  --type 'Demo::Sensor=/path/to/sensor.idl'
```

Every REST request needs `Accept: application/dds-web+xml` (Spec
§8.3.5 Tab 7 — the only representation this gateway serves; a missing
`Accept` is 400, one naming no served representation is 406, and a
`Content-Type` naming anything else is 415 —
`crate::rest_service::check_representation_headers`). A request with a
body also names it as `Content-Type`: `curl -d` alone sends
`application/x-www-form-urlencoded`, which is 415. The examples below set
both once and pass them via `-H`.

### REST: create an application, a participant, a topic, write a sample

```bash
API=http://127.0.0.1:8080/dds/rest1
ACCEPT='Accept: application/dds-web+xml'
CTYPE='Content-Type: application/dds-web+xml'

# 1. Application
curl -si -X POST "$API/applications" -H "$ACCEPT" -H "$CTYPE" \
  -d '<application><name>app1</name></application>'
# -> 201 Created, Location: applications/app1

# 2. DomainParticipant on domain 0
curl -si -X POST "$API/applications/app1/domain_participants" -H "$ACCEPT" -H "$CTYPE" \
  -d '<domain_participant><domain_id>0</domain_id></domain_participant>'
# -> 201 Created, Location: .../domain_participants/p1

# 3. Topic, naming a type this daemon's `types:` config resolved
curl -si -X POST "$API/applications/app1/domain_participants/p1/topics" -H "$ACCEPT" -H "$CTYPE" \
  -d '<topic><name>Sensor</name><type_name>Demo::Sensor</type_name></topic>'

# 4. Publisher + DataWriter
curl -si -X POST "$API/applications/app1/domain_participants/p1/publishers" -H "$ACCEPT"
# -> Location: .../publishers/pub1
curl -si -X POST "$API/applications/app1/domain_participants/p1/publishers/pub1/data_writers" -H "$ACCEPT" -H "$CTYPE" \
  -d '<data_writer><topic_name>Sensor</topic_name></data_writer>'
# -> Location: .../data_writers/dw1

# 5. Write a sample — the body is the sample itself, converted through
#    the type's own descriptor (crate::dynxml), never an opaque guess.
curl -si -X POST "$API/applications/app1/domain_participants/p1/publishers/pub1/data_writers/dw1" -H "$ACCEPT" -H "$CTYPE" \
  -d '<data><sensor_id>7</sensor_id><value>3.5</value></data>'
# -> 200 OK (a write creates no new resource, unlike the POSTs above)

# 6. Update the DataWriter's QoS in place (Spec §7.4.5.2) — merged onto
#    its current QoS (crate::qos_update), so a <qos> naming only
#    DEADLINE leaves every other policy untouched. A <qos_profile> name
#    instead of/beside <qos> resolves through the daemon's own
#    QosLibrary/QosProfile metadata store.
curl -si -X PUT "$API/applications/app1/domain_participants/p1/publishers/pub1/data_writers/dw1" -H "$ACCEPT" -H "$CTYPE" \
  -d '<data_writer><qos><deadline><period><sec>2</sec><nanosec>0</nanosec></period></deadline></qos></data_writer>'
# -> 204 No Content
```

### REST: subscribe and read

```bash
curl -si -X POST "$API/applications/app1/domain_participants/p1/subscribers" -H "$ACCEPT"
# -> Location: .../subscribers/sub1
curl -si -X POST "$API/applications/app1/domain_participants/p1/subscribers/sub1/data_readers" -H "$ACCEPT" -H "$CTYPE" \
  -d '<data_reader><topic_name>Sensor</topic_name></data_reader>'
# -> Location: .../data_readers/dr1

curl -si "$API/applications/app1/domain_participants/p1/subscribers/sub1/data_readers/dr1" -H "$ACCEPT"
# -> 200 OK, <read_sample_seq><data>...</data>...</read_sample_seq>
```

### WaitSet: long-poll for new data (Spec §7.4.9)

```bash
curl -si -X POST "$API/applications/app1/waitsets" -H "$ACCEPT" -H "$CTYPE" \
  -d '<waitset><name>ws1</name><readers><item>applications/app1/domain_participants/p1/subscribers/sub1/data_readers/dr1</item></readers></waitset>'

# Blocks up to ?timeout=<seconds> (default 10) for a condition to
# trigger; an empty <list/> means the timeout elapsed, not an error.
curl -si "$API/applications/app1/waitsets/ws1?timeout=30" -H "$ACCEPT"
```

### SOAP: the same object model, a SOAP envelope

```bash
curl -si -X POST http://127.0.0.1:8080/soap \
  -H 'Content-Type: application/soap+xml' \
  -d '<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope">
        <soap:Body>
          <CreateApplication><name>app1</name></CreateApplication>
        </soap:Body>
      </soap:Envelope>'
```

`WriteSample`/`ReadSample`'s `data` field carries the sample XML, the
same representation REST uses, so a client that already builds REST
bodies reuses that code for SOAP.

### WSDL

```bash
curl -s http://127.0.0.1:8080/wsdl
```

Lists every DDS-WEB object-model operation (`CreateApplication`,
`WriteSample`, ...) plus every operation of every `interface` in a
config-named `interfaces:` IDL file. A business interface's own
operation is dispatched over SOAP too — `crate::rpc_dispatch` builds
its `In`/`Out` structs from the operation's parameters (DDS-RPC 1.0
§7.5.1.1.4/§7.5.1.1.5), sends the request through a `Requester` on the
daemon's own DDS-RPC client `DomainParticipant` (`domain:`), and
converts the reply back to the SOAP body the same way `crate::dynxml`
converts a topic sample. WSDL and runtime read the same
`zerodds_lang_rules::adapter::interface` model, so an operation the
WSDL lists is always one the daemon can dispatch, and one it cannot
(neither a builtin nor a configured interface's operation) returns a
named `env:Receiver` fault, never a silent no-op.

DDS-RPC 1.0 §7.4.3/§7.5.1.1 topic granularity: one request/reply topic
pair per **interface** (`service_name` = the interface's qualified
name, e.g. `demo_Calculator`), shared across all its operations — the
Basic mapping's `Call`/`Return` union, keyed per operation by
`operation_hash` (§7.5.1.1.4), with a per-operation `Result` union
carrying either the `Out` struct or a declared `raises` exception
(§7.5.1.1.5). SOAP dispatch selects the union branch for the named
operation; every synthesized wire type is `@final` (classic-CDR
shape, no `DHEADER`), matching what `zerodds-idlc generate --lang
rust`/`--lang cpp` themselves emit for an `interface`
(`crates/lang-rules/src/emit/interface_rust.rs`). `tests/soap_rpc_e2e.rs`
proves this against a **real** `zerodds-idlc`-generated `Replier`
(`zerodds-rpc-rust-e2e`'s `Calculator` binding, on a second, independent
`DomainParticipant`) — not a hand-written stand-in — so interoperability
with a strictly spec-conformant generated client/server pair is a
measured property, not an assumption.

## Configuration

See `crates/webdds-bridge/src/daemon/config.rs` for the full schema
(`man 5 zerodds-webdds-bridged.yaml`) and
`packaging/linux/configs/webdds-bridged.yaml.example` for a worked
example.

## Open points, closed

Three points this README used to list as open (documented, not silent
gaps) are closed as of this round; each is kept here with what closed
it, per project convention (a claim of "done" names its evidence).

- **DDS-RPC topic granularity.** Was: one topic pair per operation.
  Now: one topic pair per *interface*, DDS-RPC 1.0 §7.4.3/§7.5.1.1, as
  the "WSDL" section above describes. Closed by `crate::rpc_dispatch`'s
  rewrite and `tests/soap_rpc_e2e.rs::soap_add_reaches_a_generated_replier_and_returns_its_reply`/
  `soap_div_by_zero_surfaces_the_declared_exception`, both against a
  real `zerodds-idlc generate --lang rust` binding
  (`zerodds-rpc-rust-e2e`'s `Calculator`), not a hand-written `Replier`.
- **`update_*`'s `<qos_profile>name</qos_profile>` reference and
  contained-entity sync.** Was: only an inline `<qos>` resolved;
  `create_participant`/`create_publisher`/`create_subscriber` read no
  contained entities, and their `update_*` counterparts synced none.
  Now: `crate::qos_update::resolve_entity_qos`/`resolve_qos_profile_ref`
  resolve a `<qos_profile>` reference through the daemon's own
  `QosLibrary`/`QosProfile` metadata store (override-wins merge with an
  inline `<qos>`, Spec's own precedent for QoS composition);
  `crate::contained_entities` parses nested `<publishers>`/
  `<subscribers>`/`<data_writers>`/`<data_readers>`
  (§7.4.3.1/§7.4.3.2/§7.4.4.7/§7.4.4.8/§7.4.4.11/§7.4.4.12);
  `crate::gateway`'s `sync_publishers`/`sync_subscribers`/
  `sync_data_writers`/`sync_data_readers` add/re-QoS/remove by name on
  every `create_*`/`update_*` call, gating on whether the container
  element is *present* (absent = leave untouched, empty = delete every
  contained entity of that kind) rather than on whether QoS was named.
  Closed by `crates/webdds-bridge/src/qos_update.rs` and
  `crates/webdds-bridge/src/contained_entities.rs`'s own test modules,
  plus `crate::gateway`'s
  `update_participant_creates_and_reconciles_contained_entities` e2e
  test (real DCPS entities, offline domain).
- **Optional data representations / content negotiation.** DDS-WEB 1.0
  §8.2/§8.3.5 (Tab 7/Tab 8) define exactly one representation,
  `application/dds-web+xml` — confirmed by full-text reading of the
  spec PDF; there is no JSON representation to implement. What *was*
  open: the media type string was wrong everywhere in this codebase
  (`application/zerodds-web+xml`, fixed in six files — spec conformance
  bug, not a design choice), and neither `Accept` nor `Content-Type`
  was checked at all — a request naming any other representation, or
  none, was silently served anyway. Closed by
  `crate::rest_service::check_representation_headers` (400 on a missing
  `Accept`, 406 on one naming no served representation, 415 on a
  `Content-Type` naming one), wired into `dispatch()` ahead of every
  other check, and its own test module's four new tests.

## Security

Reuses the bridge fleet's common substrate, `zerodds-bridge-security`
(TLS, `auth.mode: none|bearer|jwt|mtls`, an ACL), rather than
`zerodds_web::access_control`'s own §7.3 `AccessController`/
`Permissions` engine — both model the same Spec §7.3 decision shape,
and this daemon follows the fleet's shared implementation for
consistency (uniform config schema, one crate for a CVE fix to land
in). The ACL's resource unit is the **Application name**, not the DDS
topic name `zerodds_bridge_security::Acl` is shaped for — see
`daemon::config`'s module doc for why.

## Testing

```bash
cargo test -p zerodds-webdds-bridge                       # 96 unit tests
cargo test -p zerodds-webdds-bridge --test daemon_e2e      # 4 integration tests
cargo test -p zerodds-webdds-bridge --test soap_rpc_e2e     # 2 integration tests (SOAP↔DDS-RPC↔Replier)
cargo test -p zerodds-webdds-bridge --test example_yaml_loadback  # 2 integration tests
```

`daemon_e2e.rs` runs the daemon in-process on a leased DDS domain,
drives it over a real `TcpStream`, and verifies a write with a
**second, independent** `DomainParticipant` that receives the sample
over real RTPS/UDP — not through the daemon's own bookkeeping.
