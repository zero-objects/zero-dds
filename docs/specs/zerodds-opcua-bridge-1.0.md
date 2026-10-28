# `zerodds-opcua-bridge` v1.0 — the OMG DDS-OPCUA Gateway 1.0 daemon

Implementation: `crates/opcua-bridge` (binary `zerodds-opcua-bridged`), on
top of the four library crates `zerodds-opcua-gateway`,
`zerodds-opcua-uacp`, `zerodds-opcua-server`, `zerodds-opcua-pubsub`.

## Motivation

The four library crates implement the OMG **DDS-OPCUA Gateway 1.0**
(formal/2020-01-01) type system and AddressSpace mapping, the OPC-UA
**Part 6** `opc.tcp` framing and SecureChannel, the **Part 4** Client/Server
service set, and the **Part 14** (UADP) PubSub wire stack — as libraries,
with no DDS runtime attached, no network listener loop, and no
configuration file. This crate is the missing part: a running process a
DDS deployment and an OPC-UA deployment can actually be bridged with.

## §1 Conformance levels

This daemon declares all four Gateway 1.0 §2 conformance points
(`zerodds_opcua_gateway::conformance::DECLARED_CONFORMANCE`) at the library
level; at the daemon level it wires:

- §9 (DDS → OPC UA): `dds_to_ua_connection` topics materialized into this
  daemon's own `opc.tcp` AddressSpace, kept live by a DDS reader.
- §8 (OPC UA → DDS): both directions — a `ua_to_dds_connection` node a
  third-party OPC-UA client writes to, and a configured foreign OPC-UA
  server this daemon browses/subscribes as a client.
- Part 14 UADP PubSub, both directions, over UDP multicast, optionally
  secured (§4.3).
- Historical access (§9.3.4, conformance point 4 "complete") is wired via
  `opcua.server.historical.topics` — see §5.4.

## §2 CLI surface

```
zerodds-opcua-bridged [OPTIONS]

  --config FILE             base config file (default: system config dir)
  --check-config            validate + report, then exit (0 valid, 1 invalid, 2 unreadable)
  --set PATH=VALUE          one setting above every file; +PATH=VALUE appends to a list
  --bind ADDR               opc.tcp bind address (default 0.0.0.0:4840)
  --domain ID               DDS domain ID (default 0)
  --idl FILE                IDL file to compile (repeatable)
  --gateway-config FILE     Spec §10 XML gateway config
  --log-level LEVEL         trace/debug/info/warn/error
  --metrics ADDR            Prometheus listener (enables metrics)
  --version | --help
```

Every convenience flag is `--set` of its file key (`--bind X` is `--set
opcua.server.bind=X`), applied after the files, so it always wins.

## §3 Config-file format

YAML, layered: defaults < environment < files (`opcua-bridged.yaml` +
`opcua-bridged.d/*.yaml`) < `--set`. An unknown key inside a known section,
or a value of the wrong shape, stops the start with file and line; see
`zerodds-opcua-bridged.yaml(5)` for the full key list.

```yaml
domain: 0
log_level: info

opcua:
  idl: ["/etc/zerodds/opcua/robotics.idl"]
  gateway_config: "/etc/zerodds/opcua/gateway.xml"   # Spec §10 XML

  server:
    enabled: true
    bind: "0.0.0.0:4840"
    security_policies: ["none"]
    historical:
      topics:
        - dds_topic: "Telemetry"
          depth: 1000        # per-leaf retention bound (Spec §9.3.4.4)

  client:
    enabled: false
    servers:
      - endpoint_url: "opc.tcp://plc.local:4840"
        nodes:
          - node_id: "ns=2;s=Cmd"
            dds_topic: "Commands"
            dds_type: "robotics::Command"
            field_path: "value"

  pubsub:
    enabled: false
    connections:
      - name: "primary"
        multicast_addr: "224.0.0.22:4840"
        publishers:
          - dds_topic: "Telemetry"
            writer_group_id: 1
            dataset_writer_id: 1
        subscribers:
          - dds_topic: "Commands"
            fields: ["value"]
        security:                          # Part 14 §8, feature `pubsub-security`
          policy: "aes256-gcm"              # aes128-ctr | aes256-ctr | aes256-gcm
          security_group_id: "g1"
          token_id: 1
          encrypting_key: "<64 hex chars>"   # signing_key empty for aes256-gcm
          key_nonce: "<8 hex chars>"

acl:
  default_deny: false
  allow_anonymous: true
  users:
    - username: "opc-client-1"
      password: "s3cret"
  rules:
    - subject: "opc-client-1"
      allow_read: ["Telemetry"]
      allow_write: ["Commands"]

metrics:
  enabled: false
  listen: "127.0.0.1:9095"
  otlp_endpoint: "http://127.0.0.1:4318"
```

## §4 Wire protocols

### §4.1 `opc.tcp` (Part 6 §7.1) + SecureChannel (§6.7)

This daemon's own server surface. SecurityMode `None` always works. With
the `crypto` build feature, `basic256sha256` / `aes128_sha256_rsaoaep` /
`aes256_sha256_rsapss` are available — see §7 for the certificate/key
material shape this daemon needs.

### §4.2 Part 4 services

Session lifecycle, Read/Write (Attribute), Browse (View), Call (Method),
CreateSubscription/CreateMonitoredItems/Publish (Subscription), and
HistoryRead (§5.4) — the full set `zerodds-opcua-server` implements. One
`Server` per daemon process, shared by every accepted `opc.tcp` connection
(Part 4 §5.6.2 allows several concurrent SecureChannels and a Subscription
can be transferred between them, so connections cannot each own a private
`Server`).

`Server::set_write_hook` (new since rc.7) offers every `Write` target to a
caller-installed closure, with the writing session's identity, before it
ever reaches the AddressSpace. `zerodds-opcua-bridged` installs one that
enforces `acl.rules[].subject` per session (§7.2) — the first synchronous
per-session check this daemon has had; the actual DDS-forwarding path for
a successful write is still the poll loop (§6.2), unchanged.

### §4.3 Part 14 UADP

One `Publisher`/`Subscriber` pair per configured `opcua.pubsub.connections[]`
entry, over `UdpTransport` (unicast/multicast UDP). Message security
(`SecurityGroup`/SKS, feature `pubsub-security`) is wired via
`opcua.pubsub.connections[].security`: a single static current key
(`policy`, `security_group_id`, `token_id`, `signing_key`/`encrypting_key`/
`key_nonce` as hex) — not a live `GetSecurityKeys`/SKS rotation exchange;
an operator rotates the key by reloading config with a new key/`token_id`.
A hex-decoded key whose length disagrees with its policy is refused at
load time.

## §5 Topic mapping

### §5.1 Types: TypeObject/TypeLookup → `SerializedTypeSupport`

A topic's DDS type is never a compiled Rust type here. `opcua.idl` files
are compiled (`zerodds-idl-compose`) into a model; a `DynamicType` for the
type a config entry names is turned into a `CompleteTypeObject`
(`zerodds_types::from_ast::complete_type_object`) and then a
`zerodds_dcps::serialized_type::SerializedTypeSupport` — the same
reflective decoder/encoder `crates/xrce-agent`'s daemon and
`crates/routing-service` use for a topic with no compiled type. This is the
"types via TypeObject/TypeLookup" path.

A second, **opportunistic** path exists: `crate::dds_bridge::
resolve_discovered` looks a SEDP-discovered remote endpoint's
`TypeIdentifier` up in the runtime's own TypeLookup registry
(`DcpsRuntime::type_lookup_server`), which is filled by
`DcpsRuntime::register_type_object` and by a `getTypes` reply the runtime
received for some other reason. This daemon does **not** itself drive a
live cross-participant `getTypes` RPC exchange for a topic whose type is
otherwise unknown — see §6.2.

### §5.2 AddressSpace shape (§9.3)

`DomainNode` (`Domain<id>`, under the `Objects` folder) → `TopicNode`
(`Topic:<name>`) → the sample-variable tree
(`build_sample_instance`/`InstanceNode`, materialized under
`Topic:<name>/Value[/<dotted member path>]`). NodeId generation is spec
"vendor-defined"; this daemon's own scheme is the default. The XML
`<browse_path>`/`<node_id>` override Spec §10 allows is **applied**: a
`<node_id>` replaces the connection's sample root's identity (checked
against every existing node first — a collision is a load-time error, the
daemon never starts under it); a `<browse_path>` adds an `Organizes`
reference chain from the well-known `Objects` folder (ns0;i=85) to the
same root. Both are additive to the default `Topic:<name>` reachability,
never a replacement of it; an empty `<browse_path>` segment is likewise a
load-time error, never a log line.

A `dds_to_ua_connection` leaf's `AccessLevel` is `CurrentRead` only, so a
foreign client's `Write` against it fails `Bad_NotWritable` rather than
succeeding with no effect. A `ua_to_dds_connection` leaf is `CurrentRead |
CurrentWrite`.

### §5.3 Type-shape coverage

Primitives, strings (narrow and wide — `wchar`/`wstring` map to
`UInt16`/`String`, mirroring `dds_to_ua::walker`'s type-level mapping),
enumerations (as their ordinal), bitmasks (as a bit-position-packed
integer), nested structs, `map<K,V>` (one leaf per entry, path-named
`<field>.<key>`; a map-of-struct decomposes further, same as any nested
aggregate), and arrays/sequences of any of the above are mapped, both
directions.

An array/sequence of a struct/union, a `bitset`, `fixed<P,S>` and
`float128` (`long double`) are **not** mapped to a live Value — reported
by path (never silently dropped) and left as a browsable, valueless node.
For the first, Gateway 1.0 §9.2.5's own "array of struct is one array
Variable, not decomposed" design boundary applies, and `VariantValue` has
no aggregate case to hold one anyway; for the other three, no OPC-UA
builtin `VariantValue` case fits at all (Part 6 §5.1.2's type table has no
row for any of them) — inferred from that absence, not a specific Gateway
1.0 §9.2 clause this crate has the text of.

A `map` key that a write introduces and no prior sample ever had is
reported as an unmatched path (the same mechanism any shape-drift already
uses) rather than materializing a new node/field live —
`AddressSpace`/`PublishedDataSet` build their node/field set once, from
the topic's Table-9 default; dynamic node/field creation is a tracked
follow-up, not implemented.

### §5.4 Historical access (§9.3.4)

`opcua.server.historical.topics[]` (`dds_topic`, `depth`) marks every leaf
of a `dds_to_ua_connection` topic historizing: `Historizing = true`, the
`HistoryRead` `AccessLevel` bit set, and up to `depth` samples retained per
leaf (`InMemoryHistoryCache::push_bounded`, evicting the oldest by
timestamp). `zerodds-opcua-server::Server` answers Part 4 §5.10.3
`HistoryRead` (Raw range and AtTime — DDS has no modified-value history, so
a `ReadRawModifiedDetails.is_read_modified == true` request is refused,
not silently answered as a plain read) with real pagination via an opaque
continuation point. A `dds_topic` named here that matches no
`dds_to_ua_connection` (or only a `ua_to_dds_connection`, where historizing
has no meaning) is a load-time error. The DDS `INFO_TS` source timestamp
(falling back to this process's receipt-time wall clock when a sample
carries none) orders retained samples — an earlier version of this pump
used a constant `0`, which collapsed every node's retention to one sample;
fixed.

## §6 OPC UA ↔ DDS bridging behavior

### §6.1 DDS → OPC UA (§9)

Every `dds_to_ua_connection` topic gets a DDS reader; each received sample
is decoded, flattened to named leaf values, and applied to the AddressSpace
via `AddressSpace::set_value`. Best-effort delivery — a sample that never
matches the reader (e.g. QoS-incompatible) never reaches a value, same as
any DDS DataReader.

### §6.2 OPC UA → DDS (§8)

Two independent mechanisms:

1. **A third-party client writes to this daemon's own server.** The Write
   first passes `Server::set_write_hook`'s installed closure (§4.2), which
   enforces `acl.rules[].subject` against the writing session's real
   identity (§7.2) — a denial is `Bad_UserAccessDenied`, synchronously,
   before the AddressSpace is ever touched. A write the hook allows still
   reaches DDS the same way it always has: a `ua_to_dds_connection` leaf's
   current Value is polled every 100 ms against its last-seen value
   (`crate::daemon::server`'s pump loop); a change is re-encoded and
   written into DDS. This remains **poll-based**, not synchronous — an
   *allowed* write's result (the client's own Write response) is still not
   contingent on the DDS write actually landing; a DDS write failure is
   logged, never silently swallowed, but the OPC-UA client that wrote is
   not told about it. This narrower limitation (only about the DDS side of
   an already-authorized write) remains, named, not fixed in this pass.
2. **This daemon browses/subscribes a configured foreign OPC-UA server as a
   client** (`opcua.client.servers[]`), one `Client`/`TcpTransport`/
   Subscription per entry, one MonitoredItem per configured node; each
   notification is written into its target DDS topic.

`acl.*` gates **topic exposure at config-load time** for both directions
(§7.2) *and*, for direction 1, **per-OPC-UA-session identity** via the
Write hook — the coarser, config-time gate is not a replacement for the
finer one: a topic no rule names for an operation at all is never
registered in the first place, whichever subject later asks.

## §7 Security

### §7.1 SecureChannel (feature `crypto`)

`opcua.server.security_policies` names the secured policies to accept.
`certificate_der`/`private_key_pem` are this daemon's own material;
`trusted_client_certificate_der`/`trusted_client_public_key_pem` are the
one trusted peer's. **Why two files for the peer, not one certificate**:
`zerodds_opcua_uacp::crypto::ServerSecurity` needs the client's certificate
(sent on the wire as-is) *and* its RSA public key (to verify signatures) as
two separate values; this daemon has no X.509 parser to split a single
certificate into both, so the operator supplies them separately. A real
operational rough edge, not hidden.

`zerodds-bridge-security`'s TLS loader (`rustls::ServerConfig`) is not used
here — a different shape than the bare RSA keypair the SecureChannel needs.

### §7.2 ACL

`acl.rules`: `(subject, allow_read[], allow_write[])`. Two layers, both
enforced: topic exposure at config-load time (a topic no rule names, when
any rule exists or `default_deny: true`, is never materialized/registered
at all, for any client) and, for a Write against this daemon's own server
(§6.2), per-OPC-UA-session identity via `Server::set_write_hook`.

`acl.allow_anonymous` (default `true`) and `acl.users[]`
(`policy_id`/`username`/`password`) give this daemon's `UserStore` real,
distinguishable session identities — without a named user, every session
is Anonymous and `acl.rules[].subject` has nothing to differ on for the
Write hook. A session's subject string (`client_subject`) is `"anonymous"`
for Anonymous, the username for a UserName token, or the raw
certificate/issued-token bytes hex-encoded.

## §8 Operations + observability

### §8.1 Logging

`log_level` (`trace`/`debug`/`info`/`warn`/`error`) to stderr; every skip,
decode failure and DDS-write failure is logged by name (topic/path), never
silent.

### §8.2 Prometheus metrics

`metrics.listen` (default `127.0.0.1:9095` when `metrics.enabled` and no
address given) serves `GET /metrics`:
`zerodds_opcua_{dds_to_ua,ua_to_dds}_samples_total`,
`zerodds_opcua_pubsub_{published,received}_total`,
`zerodds_opcua_connections_total`, `zerodds_opcua_errors_total`. This
daemon runs its own minimal server (`crate::daemon::server::serve_metrics`)
rather than `zerodds_monitor::server::serve_prometheus`, whose accept loop
has no working shutdown path (measured: `admin_thread` never rejoins after
SIGTERM against that helper) — see the function's own doc comment.

### §8.3 OTLP spans

`metrics.otlp_endpoint` (an OTel collector's OTLP/HTTP base URL) exports
coarse-grained lifecycle spans (`zerodds_foundation::tracing`'s own design
line: "endpoint lifecycle and discovery phases, not per-sample
latencies") — one `zerodds-opcua-bridged.opc_tcp.accept` span
(`SpanKind::Server`, the peer address as an attribute) per accepted
`opc.tcp` connection. Flushed every 5 seconds and once more on shutdown.
Trace/span IDs are practically-unique (a timestamp plus a process-wide
counter), not cryptographically random.

## §9 Lifecycle

### §9.1 Startup

Compiles `opcua.idl`, parses `opcua.gateway_config` (if any), starts one
`DcpsRuntime` for `domain`, materializes every topic's AddressSpace tree,
connects every configured client/PubSub job, then binds `opc.tcp` and (if
enabled) the metrics listener.

### §9.2 Shutdown

SIGTERM/SIGINT set a stop flag; the `opc.tcp` accept loop wakes via a
self-connect, the metrics loop via non-blocking polling of the same flag,
and every worker thread is joined before the process exits (`ExitCode 0`).

### §9.3 SIGHUP

Re-reads `acl.default_deny`/`acl.rules` from the same layered sources the
process started with; a file that fails to load changes nothing.
`domain`, `opcua.server.bind`, the IDL/gateway-config-driven topic set
(including any `<node_id>`/`<browse_path>` override and
`opcua.server.historical.topics`), `acl.users`/`acl.allow_anonymous`,
`opcua.pubsub.connections[].security` and `metrics.otlp_endpoint` all need
a restart — SIGHUP's scope has not widened past `acl.default_deny`/
`acl.rules`.

## §10 Cross-vendor interop

The libraries are measured against **open62541 1.5.4** (`/root/o62-ex`,
`/root/o62-ex2` on the Linux measurement host, built by
`crates/opcua-pubsub/proofs/build-open62541.sh`) — see the crate README for
the exact commands. At the daemon level,
`crates/opcua-bridge/tests/open62541_interop.rs` (gated on
`OPCUA_OPEN62541=1`; see `crates/opcua-bridge/tests/foreign/README.md` for
prerequisites) drives a real running `zerodds-opcua-bridged` from a tiny C
client built against `libopen62541`'s own public API — Browse the
`Objects` folder, Read a `dds_to_ua_connection` leaf, Write a
`ua_to_dds_connection` leaf and confirm the write reached DDS. Not run in
this daemon's own CI; a generic OPC-UA client (UaExpert or any
GetEndpoints/Browse/Read/Subscribe-capable tool) can drive the `opc.tcp`
server the same way.

## §11 Packaging

Package `zerodds-opcua-bridge`; systemd unit
`packaging/linux/systemd/zerodds-opcua-bridged.service` (`Type=notify`);
example config `packaging/linux/configs/opcua-bridged.yaml.example`; man
pages `zerodds-opcua-bridged(1)` + `zerodds-opcua-bridged.yaml(5)`; deb/rpm/
Arch/AppImage entries alongside the other bridge daemons.

## §12 Testing

### §12.1 Unit tests

Per-module, `cargo test -p zerodds-opcua-bridge --lib`: the `DynamicType` ↔
`DdsType` conversion, `DynamicData` ↔ `Variant` flatten/unflatten, the IDL
compiler, the DDS port, the AddressSpace materialization, the PubSub glue,
the OPC-UA client glue, and the daemon's own CLI/config/ACL/security
modules.

### §12.2 Integration (in-process e2e)

`crates/opcua-bridge/tests/daemon_e2e.rs`, against the real
`server::start_with_reload`:

1. a DCPS writer → the daemon → an in-tree OPC-UA `Client` reads the value;
2. an in-tree OPC-UA `Client` `Write` → the daemon → a DDS reader sees it;
3. a DCPS writer → the daemon's Part-14 `Publisher` → an in-tree
   `zerodds-opcua-pubsub` `Subscriber` over real UDP multicast receives it;
4. a Spec §10 XML `<node_id>`/`<browse_path>` override is applied, and a
   colliding `<node_id>` refuses to start;
5. `opcua.server.historical.topics` makes `HistoryRead` answer real
   retained samples;
6. per-session `acl.rules` allows a named subject's Write and refuses
   another's, both through the real Write hook;
7. `metrics.otlp_endpoint` delivers a real span to a fake OTLP collector
   on an `opc.tcp` accept;
8. a Basic256Sha256 SignAndEncrypt session, with real RSA-signed X.509
   certificates generated in the test, through the real daemon;
9. an external Part-14 `Publisher` → the daemon's own
   `subscribers[]`-configured `SubscriberJob` → an in-tree DDS reader
   sees it — the inverse of test 3, and previously untested.

Test 3 is measured to be unreliable over macOS loopback multicast; it
reports plainly instead
of asserting on that host family. Measure on Linux for a firm result. This
crate's DDS-domain-lease e2e tests are also measured to be flaky under
`cargo test`'s default parallelism (each passes individually and serially;
run `-- --test-threads=1` for a firm result).

A Linux measurement of the merged round-1 work found test 3 failing
outright (no datagram within 15 s). Root cause, found and fixed: every
UDP multicast *receiver* socket in this crate bound to
`opcua.pubsub.connections[].local_addr`'s own port (`0.0.0.0:0` by
default) rather than the multicast destination's port — UDP
demultiplexes by destination (ip, port) at the kernel, so a socket
bound to an arbitrary port never receives a datagram addressed to the
real one. This affected both the test's own hand-built verification
`Subscriber` (test 3) and the daemon's own `subscribers[]`-configured
`SubscriberJob` (§6.2's other direction — untested by anything before
test 9 above), and happened to still work on macOS, which is not a
portable guarantee. Fixed: the subscriber-role bind is now
`SocketAddr::new(local.ip(), multicast.port())`, keeping `local`'s IP
(for an operator who named a specific receive interface) but always the
multicast group's own port.

### §12.3 Multi-vendor

`crates/opcua-bridge/tests/open62541_interop.rs` (§10) drives a real
running daemon from a tiny C client against `libopen62541`; gated on
`OPCUA_OPEN62541=1` and not automated in CI (same gap the review noted for
the other five bridges' cross-vendor tests). `crates/opcua-server/proofs/
open62541.sh` and `crates/opcua-pubsub/proofs/open62541_uadp.sh` are the
library-level open62541 measurements this daemon's own README points at.

## §13 Cross-references

- `docs/spec-coverage/dds-opcua-1.0.{md,en.md}` — the library's own §8/§9
  coverage.
- `docs/spec-coverage/opcua-client-server-1.05.{md,en.md}` — Part 4/6.
- `docs/spec-coverage/opcua-pubsub-1.05.{md,en.md}` — Part 14.
- `docs/spec-coverage/zerodds-opcua-bridge-1.0.{md,en.md}` — this spec's
  own coverage.

## §14 Versioning

v1.0 — first release. Round 2 closed the items named in the first pass:
the XML `<node_id>`/`<browse_path>` override (§5.2), historical access
(§5.4), per-session Write ACL (§4.2/§7.2), PubSub message security
(§4.3), OTLP spans (§8.3), `wchar`/`wstring`/`map` value mapping (§5.3),
a secured-SecureChannel e2e test with real certificates (§12.2), and an
open62541 daemon-level interop measurement (§12.3/§10). Still open: an
array/sequence of a struct/union, `bitset`, `fixed<P,S>` and `float128`
have no live-value mapping (§5.3, a design boundary, not a gap to close);
dynamic per-map-key node/field creation is not implemented (§5.3); the
poll-based DDS-forwarding path for an *authorized* Write is unchanged
(§6.2); PubSub message security is a static key, not a live SKS rotation
exchange (§4.3).
