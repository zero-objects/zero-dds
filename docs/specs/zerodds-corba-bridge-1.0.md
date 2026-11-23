# `zerodds-corba-bridge` v1.0 — DDS↔CORBA bridge daemon

ZeroDDS vendor spec. Specifies the behavior of a configurable
daemon that couples DDS topics with CORBA IIOP servers (GIOP/IIOP, OMG
formal/2012-11-14).

## Motivation

CORBA is active in the financial sector, in telco switching stacks, in air traffic
control systems, and in industrial legacy software from the early
2000s until today. Migrating such legacy systems to modern
DDS-based real-time messaging is a multi-year program — until
then, bridges are needed. The `zerodds-corba-bridged` daemon is the
coexistence component: CORBA operations are mapped to DDS request/reply
topic pairs; DDS publications are emitted as CORBA notify
events when needed.

Complementary to the libraries `crates/corba-dds-bridge/`,
`crates/corba-iiop/` (TCP transport), `crates/corba-giop/` (frame
codec) — the libraries are building blocks, this daemon is the
executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks GIOP 1.0/1.1/1.2 (OMG formal/2012-11-14) including Request/Reply/CancelRequest/LocateRequest/Fragment + IIOP 1.0/1.1/1.2 profile. |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional: GIOP request→DDS request topic + DDS reply topic→GIOP reply (request-reply pattern); optionally notification-service-style pub/sub. |
| **L4 — Config** | Operation map via YAML config (RepoId↔DDS topic pairs); hot reload via SIGHUP optional. |
| **L5 — Auth** | TLS over IIOP (SSLIOP) + CSIv2 (`SAS_ContextElement`) + GSSUP username/password. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per IIOP connection: an SSLIOP session is bound to the tenant its client certificate names, for its life (§3 `tenants`). |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

```
zerodds-corba-bridged [OPTIONS]

Options:
  --config <FILE>              Base file (YAML subset of zerodds-config),
                               plus FILE.d/; without it: corba-bridged.yaml and corba-bridged.d/
                               in the system directory
  --set <PATH=VALUE>           one value above all files, repeatable
  --check-config               check the configuration, print every value with its
                               origin, exit (0 valid, 1 invalid)
  --iiop-bind <ADDR>          IIOP bind (default 0.0.0.0:6833)
  --ssliop-bind <ADDR>        SSLIOP bind (default 0.0.0.0:6834)
  --domain <ID>               DDS domain ID (default 0)
  --orb-id <ID>               ORB identity (default "zerodds-corba-bridge")
  --tls-cert <FILE>           TLS server cert (PEM); enables SSLIOP
  --tls-key <FILE>            TLS server key (PEM)
  --topic <REPO_ID=TOPIC>     single-mapping override (several times):
                               request topic TOPIC, reply topic TOPIC::Reply
  --dump-iors                 print the IOR lines of §4.1 and exit
  --log-level <LEVEL>         trace/debug/info/warn/error (default info)
  --metrics <ADDR>            Prometheus scrape endpoint (default off)
  --version                   version info
  --help                      help

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   bind error (port in use)
  3   DDS discovery error
  4   TLS error
  5   NameService bind error
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--iiop-bind`, `--ssliop-bind`, `--domain`, `--orb-id`, `--tls-cert`/`--tls-key` (only together), `--topic <REPO_ID=TOPIC>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
and therefore win over the files. The same file may carry the keys of the
DDS runtime (`transport.*`, `discovery.*`, …), see
`zerodds-deployment-1.0.md` §2.1.

## §3 Config file format

YAML subset of `zerodds-config` (block lists, flat inline lists of
scalars, `include:`, drop-ins, `${VAR}`). An unknown key in
a known section and a value of the wrong type prevent the start,
with file and line; an unknown key at the top level produces
a warning, and under `--check-config` an error.

```yaml
# zerodds-corba-bridged.yaml
domain: 0
log_level: info

corba:
  orb_id: "zerodds-corba-bridge"
  idl_dir: "/var/lib/zerodds/corba-bridge"  # the IDL generated from idl_file (§5.2)
  iiop:
    bind: "0.0.0.0:6833"
    advertise_host: "bridge.example"  # the host of the IORs for a wildcard bind
    giop_version: "1.2"             # 1.0 | 1.1 | 1.2
    fragment_size: 65536
    max_message_size: 33554432      # 32 MB
  ssliop:
    enabled: true
    bind: "0.0.0.0:6834"
    cert_file: "/etc/zerodds/corba-cert.pem"
    key_file:  "/etc/zerodds/corba-key.pem"
    ca_file:   "/etc/zerodds/corba-ca.pem"   # asks for a client certificate
    require_client_cert: false      # true: a session without one fails
    required: false                 # true: SSLIOP only, plain IIOP forwards (§4.7)
  ior:
    ior_file: "/var/lib/zerodds/corba-bridge.ior"   # the IOR lines of §4.1
    naming_service: "corbaname::naming.example:2809#zerodds/bridge"
    publish_to_naming: false        # true: rebind every object at start, unbind at the stop
    naming_retry_ms: 30000          # retry budget while the NameService is not up
    # naming_ssliop:                # a NameService on ssliop:/tls:
    #   ca_file: "/etc/zerodds/naming-ca.pem"
    # initial_references:           # what a rir: URL names
    #   NameService: "corbaloc::naming.example:2809/NameService"
  naming:
    serve: false                    # a CosNaming::NamingContext on this listener
    persist_file: ""                # its graph, kept across restarts
  csiv2:
    enabled: false
    target_supports: [principal_name]   # identity assertion: anonymous | principal_name |
                                        # x509_cert_chain | distinguished_name
    gssup:
      password_file: "/etc/zerodds/corba-gssup.passwd"
      target_name: "zerodds"

mappings:
  # Each mapping is a CORBA-Interface-Operation ↔ DDS-Topic-Pair
  - repo_id: "IDL:zerodds/MarketData/Quote:1.0"
    operation: "request_quote"
    direction: "corba_to_dds"           # corba_to_dds | dds_to_corba | bidir
    request_topic:
      dds_name: "MarketData::QuoteRequest"
      dds_type: "MarketData::QuoteRequest"
    reply_topic:
      dds_name: "MarketData::QuoteReply"
      dds_type: "MarketData::QuoteReply"
    correlation_id: "request_id"        # name of the first member; another one needs idl_file
    # idl_file: "/etc/zerodds/idl/MarketData.idl"   # the types are generated from it (§5.2),
    #                                               # then without dds_type
    timeout_ms: 5000
    qos:
      reliability: "reliable"
      durability: "volatile"            # transient_local: the replay store of §6
      history:
        kind: "keep_last"               # keep_last | keep_all
        depth: 1
      # lifespan_ms: 30000
      # deadline_ms: 2000
      # liveliness_lease_ms: 10000
      # partition: ["site-a"]

  - repo_id: "IDL:zerodds/Trading/OrderEvents:1.0"
    operation: "on_order_event"
    direction: "dds_to_corba"
    notify_topic:
      dds_name: "Trading::OrderEvent"
      dds_type: "Trading::OrderEvent"
    targets:                          # corbaloc: or IOR strings
      - "corbaloc:iiop:1.2@listener-main:2809/TradingListener"
      - "corbaloc:iiop:1.2@listener-audit:2809/TradingListener"

# Participants beside the one of `domain` (§1 L6).
tenants:
  - name: "plant-a"
    domain: 7
    subjects: ["CN=plant-a,O=Example"]   # the client certificates bound to it
    mappings:
      - repo_id: "IDL:zerodds/Plant/Status:1.0"
        operation: "status"

acl:
  default_deny: true
  rules:
    - subject: "mtls:<SHA-256 of the client certificate, hex>"
      allow_invoke: ["IDL:zerodds/MarketData/*"]

metrics:
  enabled: true
  listen: "127.0.0.1:9096"
  path: "/metrics"
```

ENV substitution: `${VAR}` and `${VAR:-default}`. Reader:
`crates/corba-dds-bridge/src/bridge_daemon/config.rs` (`DaemonConfig::from_config`).
Rejected with a reason instead of being silently ignored:
`csiv2.enabled: true` without `csiv2.gssup.password_file`,
`ior.publish_to_naming: true` without `ior.naming_service`,
`ssliop.required: true` without `ssliop.enabled`, `correlation_id` other
than `request_id` without `idl_file`, a `dds_type` beside `idl_file`, an
`idl_file` that does not compile or lacks the interface or the operation,
an operation whose types have no DDS form (§5.2), two `idl_file`s of one
file name, `target_object_keys` (an object key names no host;
`targets` takes `corbaloc:` URLs or IORs), `qos.reliability: best_effort`
for request-reply, `qos.durability: transient_local` with
`history.kind: keep_all` and no `lifespan_ms` (an unbounded replay store),
two mappings with the same object key (§4.5), two tenants of one name or
one subject bound to two tenants, a tenant without subjects.

## §4 GIOP/IIOP wire protocol

### §4.1 IOR generation

Per mapping the daemon creates an IOR with:
- `type_id`: `repo_id` from the config
- one `IIOP-Profile` (`TAG_INTERNET_IOP`, profile tag 0):
  - `version`: according to `corba.iiop.giop_version`
  - `host`: bind hostname, `corba.iiop.advertise_host` for a wildcard bind
  - `port`: 6833; 0 with `corba.ssliop.required` (SSLIOP: a target that
    accepts no unprotected IIOP names port 0)
  - `object_key`: stable hash over `(repo_id + dds_topic_pair)` — see §4.5
  - Components (IIOP 1.1 and later): `TAG_ORB_TYPE` (component tag 0),
    `TAG_CODE_SETS` (1), with SSLIOP `TAG_SSL_SEC_TRANS` (20), with CSIv2
    `TAG_CSI_SEC_MECH_LIST` (33)
- `TAG_SSL_SEC_TRANS`: `target_supports`, `target_requires`, `port`
  (6834). SSLIOP carries its port as this component of the IIOP profile,
  not as a profile of its own.

The IOR is:
- written to `corba.ior.ior_file`, one line per mapping:
  `<repo_id> <operation> IOR:…`, a tenant's (§1 L6) as
  `@<tenant> <repo_id> <operation> IOR:…`; `--dump-iors` prints the same
  lines. When bound to `0.0.0.0`, the profile carries
  `corba.iiop.advertise_host` (environment `ZERODDS_CORBA_ADVERTISE_HOST`,
  default `127.0.0.1`).
- bound into the NameService `corba.ior.naming_service` with
  `corba.ior.publish_to_naming: true` (`corbaname:`/`corbaloc:`/`rir:`,
  over IIOP or SSLIOP; the URL syntax of CORBA 3.3 Part 2 §7.6.10: key
  optional, `NameService` for a `corbaname` without key and for an empty
  `rir` key, an IPv6 host in brackets; `ssliop:` and `tls:` take the
  address syntax of `iiop:`), retried for `naming_retry_ms`, unbound at the
  stop; a NameService that does not answer is exit 5. The objects of the
  participant of `domain` are bound; a tenant's are in `ior_file`.

`TAG_CODE_SETS`: char native UTF-8, conversion ISO 8859-1; wchar UTF-16.
`TAG_SSL_SEC_TRANS`: `target_supports` Integrity, Confidentiality,
EstablishTrustInTarget, NoProtection unless `ssliop.required`, with a CA
also EstablishTrustInClient; `target_requires` EstablishTrustInClient with
`require_client_cert`, Integrity and Confidentiality with
`ssliop.required`. `TAG_CSI_SEC_MECH_LIST`: §7.2. `TAG_ORB_TYPE` carries a
value the OMG assigns (CORBA 3.3 Part 2 §7.6.6.1); until the OMG assigns
ZeroDDS one (requested) the IOR does not carry the component, which is
optional. omniORB 4.3.3 `catior` reads the components (Linux test host).

### §4.2 GIOP frame format

Standard GIOP header (12 bytes):
```
+----+----+----+----+--------+--------+--------+------------+
|'G' |'I' |'O' |'P' | major  | minor  | flags  | msg_type   |
+----+----+----+----+--------+--------+--------+------------+
|                  message_size (4 bytes)                   |
+-----------------------------------------------------------+
```
- `flags.bit0` = byte order (0=BE, 1=LE)
- `flags.bit1` = fragment bit (GIOP 1.1+)
- `msg_type`: 0=Request, 1=Reply, 2=CancelRequest, 3=LocateRequest,
  4=LocateReply, 5=CloseConnection, 6=MessageError, 7=Fragment

The body follows with the CDR-encoded RequestHeader/ReplyHeader according to the GIOP
version, then the CDR-encoded operation args/results.

A message whose magic is not `GIOP`, whose version is not 1.0, 1.1 or 1.2,
which is larger than `max_message_size`, or which does not decode is
answered with `MessageError` (GIOP §15.4.8), in the message's version where
it is one of those, else 1.2; then the connection is closed.

### §4.3 Request-reply mapping (CORBA → DDS)

Incoming GIOP request:
1. Parse RequestHeader → `(object_key, operation, request_id)`.
2. Lookup `(object_key, operation)` → mapping → DDS request topic.
3. Args are CDR-decoded and packed into a `<dds_type>` sample.
4. The daemon publishes on the request topic with source timestamp = now.
5. The daemon creates a pending request entry under the value it wrote
   into the first member of the sample. That member is `request_id`; with
   the type of the mapping from `idl_file` (§5.2), `correlation_id` names
   it.
6. The DDS subscriber (backend service) processes the request, publishes
   the reply on the reply topic.
7. The daemon correlates the reply via `correlation_id`, sends the GIOP reply
   back with `reply_status=NO_EXCEPTION` + CDR-encoded body.

On timeout (`timeout_ms`): the daemon emits a GIOP reply with
`reply_status=SYSTEM_EXCEPTION`, `TIMEOUT (CORBA::TIMEOUT)`.

The DDS samples carry the GIOP bytes unchanged, in the byte order
of the GIOP message, as XCDR1 (whose alignment from the start of the body corresponds to that of
GIOP 1.2):

```idl
@final struct <Op>Request {
    unsigned long long request_id;   // the bridge's, unique per daemon
    // in and inout arguments, as GIOP encodes them
};
@final struct <Op>Reply {
    unsigned long long request_id;   // of the answered request
    long reply_status;               // 0 NO_EXCEPTION, 1 USER_EXCEPTION,
                                     // 2 SYSTEM_EXCEPTION (ReplyStatusType)
    unsigned long reserved;          // 0; keeps the rest 8-aligned
    // return value and out/inout, or the exception as GIOP encodes it
};
```

The correlation member is therefore always the first one; without
`idl_file` the bridge declares no type and its name is `request_id`. Because the
alignment only fits GIOP 1.2, a mapped operation over GIOP
1.0/1.1 is answered with `NO_IMPLEMENT`. Further answers: unknown
object key `OBJECT_NOT_EXIST`, wrong operation `BAD_OPERATION`, ACL
`NO_PERMISSION`, DDS write error `TRANSIENT`, an XCDR2 reply sample or an
unknown `reply_status` `MARSHAL`, a missed deadline of the reply topic
`TRANSIENT` (§6). `CancelRequest` removes the open request of its
connection; its reply sample is dropped. A oneway request is written to the
request topic and gets no reply.

### §4.4 Notify mapping (DDS → CORBA)

Per `direction=dds_to_corba` mapping:
- A DDS sample on `notify_topic` is sent as a oneway GIOP 1.2 request to every
  target in `targets` (`corbaloc:` or IOR); body = sample payload
  (XCDR1, byte order of the sample; an XCDR2 sample is discarded and
  counted). One connection per target, re-established after an error.

### §4.5 Object key generation

```
object_key = SHA-256( repo_id + "\0" + dds_topic_pair_canonical )[..16]
```
- 16 bytes = stable + collision-resistant for ≤ 2^64 mappings.
- `dds_topic_pair_canonical` = `request_topic + "->" + reply_topic` for RR
  or `notify_topic` for notify.

### §4.6 Fragment handling (GIOP 1.1+)

Messages > `fragment_size` are split into `Fragment` frames
(flag `more_fragments` in the header up to the last frame); in GIOP 1.2
each `Fragment` carries the request id, in GIOP 1.1 none does; GIOP 1.0
has no fragments, its messages go out whole. Every fragment except
the last ends on an 8-octet boundary of the message, header
included: the first message carries 12 header octets, its data is 4
shorter than a multiple of 8 (omniORB 4.3 fragments the same way, measured;
a reply cut differently made omniORB hang). Inbound,
fragments are reassembled before decoding.

### §4.7 LocateRequest/Reply

The daemon answers `LocateReply` with:
- `OBJECT_HERE` for known mappings.
- `UNKNOWN_OBJECT` otherwise.
- `OBJECT_FORWARD` with an alternative IOR on a `target_supports` mismatch:
  with `corba.ssliop.required` a client on plain IIOP gets the object's
  IOR (port 0, the SSLIOP port in `TAG_SSL_SEC_TRANS`) — to a
  `LocateRequest` as `OBJECT_FORWARD`, to a Request as a Reply with
  `LOCATION_FORWARD` (GIOP §15.4.3.2); a key nothing serves is
  `UNKNOWN_OBJECT` and `OBJECT_NOT_EXIST`.

## §5 Topic mapping

Mapping is done **per operation**, not per topic — because CORBA is
RPC-oriented. Per mapping: one request topic + one reply topic
(or one notify topic for one-way).

### §5.1 Slug algorithm for DDS topics

If not set explicitly:
- `repo_id` = `IDL:zerodds/MarketData/Quote:1.0`
- `operation` = `request_quote`
- Default request topic: `MarketData::Quote::request_quote::Request`
- Default reply topic: `MarketData::Quote::request_quote::Reply`
- Default notify topic: `MarketData::Quote::request_quote::Notify`

The first segment of the repository id is its prefix (`#pragma prefix`),
not a module.

Override via `request_topic.dds_name` / `reply_topic.dds_name`.

### §5.2 Type discovery

Without `idl_file` the bridge does not know the type of an operation: it
passes the argument bytes through in the layout of §4.3, and the DDS
backend declares request and reply type by that layout.

With `idl_file` a mapping names the IDL of its interface. At startup the
daemon reads it (a quoted `#include` resolves beside the file), looks up
the interface of `repo_id` and the operation in it (an inherited one too;
an attribute as `_get_<name>` and `_set_<name>`), and generates the DDS
types. They are the default `dds_type` of the topics; `dds_type` beside
`idl_file` is rejected. The generated IDL is written to
`<corba.idl_dir>/<file name of idl_file>` (default directory
`/var/lib/zerodds/corba-bridge`), one file per `idl_file` with every mapped
operation of it, of the daemon's mappings and the tenants'.

The file declares the types the mapped operations use as the source
declares them, at their scoped names; each raised exception with members as
a structure of its name; each interface as a module; and in it one module
per mapped operation (a leading `_` of `_get_`/`_set_` dropped):

```idl
module <Interface> { module <operation> {
    @final struct Request {
        unsigned long long request_id;      // the name of correlation_id
        // in and inout arguments
    };
    enum exception_kind { <A_B_E1>, <A_B_E2> };   // the raises list, in its order;
                                                  // the scoped name with `_`
    @final union UserException switch (exception_kind) {
        case <A_B_E1>: ::A::B::E1 as_<A_B_E1>;    // one case per exception with members
    };
    @final struct Result {
        unsigned long reserved;             // 0
        // return_value, then out and inout arguments
    };
    @final struct SystemException {
        unsigned long reserved;             // 0
        string exception_id;
        unsigned long minor_code;
        unsigned long completion_status;
    };
    @final union Outcome switch (long) {    // ReplyStatusType
        case 0: Result returned;
        case 1: UserException raised;       // `exception_kind raised` when no
                                            // exception has members; absent
                                            // without a raises list
        case 2: SystemException failed;
    };
    @final struct Reply {
        unsigned long long request_id;
        Outcome status;
    };
    @final struct Notify { /* in and inout arguments */ };   // dds_to_corba, bidir
}; };
```

In XCDR1 this is the layout of §4.3 octet for octet for a result and a
system exception, so the argument bytes pass through unchanged. A user
exception differs: the sample holds the `exception_kind` in the 32 bits
after `reply_status` and the members of the exception from octet 16, where
GIOP holds the repository id and then the members. The bridge writes the
GIOP form from the sample: the repository id of that kind, and the members
decoded by the generated type and encoded again behind it. A kind that is
none of the operation's is `MARSHAL`. A `oneway` operation has no `Reply`.

Rejected at startup, because GIOP and XCDR1 encode them differently or DDS
has no type for them: `wchar`, `wstring`, `fixed`, `any`, object
references, valuetypes, `native`, `@mutable` types; a parameter of an
anonymous type other than a primitive or `string` (declare a typedef); a
parameter with the name of the correlation member.

Reader and generator: `crates/corba-dds-bridge/src/bridge_daemon/typegen.rs`.

## §6 QoS translation

CORBA GIOP is request-reply over TCP — TCP guarantees are given.
Topic-to-topic bridging in the zerodds-rpc pattern.

| DDS QoS | CORBA behavior |
|---------|-----------------|
| Reliability `RELIABLE` | standard (only RELIABLE makes sense for RR) |
| Reliability `BEST_EFFORT` | not recommended for request-reply (refused); accepted for `direction=dds_to_corba` notify |
| Durability `VOLATILE` | normal live RPC |
| Durability `TRANSIENT_LOCAL` | the DDS endpoints are TRANSIENT_LOCAL (a backend that joins late gets the requests the writer holds), and the daemon keeps the replies per request hash (SHA-256 of operation, byte order and body): a request whose reply is kept is answered from the store, without a sample; `NO_EXCEPTION` and `USER_EXCEPTION` are kept, system exceptions are not |
| History | `qos.history`: KEEP_LAST(`depth`) keeps the replies of the last `depth` requests, KEEP_ALL all of them for `lifespan_ms`; the history of the request writer |
| Lifespan | `qos.lifespan_ms`: the lifespan of the request and notify samples, and of a kept reply; `timeout_ms` is a separate setting |
| Deadline | `qos.deadline_ms`: offered by the request writer, requested by the reply and notify readers; a reply topic without a sample for that long answers the requests that waited that long with `TRANSIENT` (completed MAYBE); a notify topic without one is logged (`deadline_missed`) once per gap |
| Liveliness | `qos.liveliness_lease_ms`: the lease of the DDS endpoints; on the CORBA side GIOP `MessageError` on a broken message, the notify connection opened again after a close |
| Partition | `qos.partition`: the partitions of the DDS endpoints |

## §7 Security

### §7.1 SSLIOP

`corba.ssliop.enabled: true` enables TLS over IIOP (SSLIOP per the OMG
SSLIOP spec) on `ssliop.bind`. With `ca_file` the server asks for a client
certificate and verifies one against it; with `require_client_cert` a
session without one fails, else it goes on without a subject. With
`required: true` the objects are reached over SSLIOP only (§4.7). The IOR
carries `TAG_SSL_SEC_TRANS` (§4.1). Measured against omniORB 4.3.3 with
`libomnisslTP4` (`tests/omniorb_e2e.rs`): a client that only allows SSL
finds the port via the IOR and calls with a client certificate.

### §7.2 CSIv2 (Common Secure Interoperability v2)

`corba.csiv2.enabled: true` enables CSIv2:
- the SAS context (`SecurityAttributeService`, service context 15) of every
  request is evaluated: an `EstablishContext` with GSSUP credentials
  against `gssup.password_file`, an asserted identity of the forms
  `target_supports` names; the authenticated GSSUP user may assert only
  itself or the anonymous identity, any other asserted identity is a
  `ContextError` with invalid evidence (CORBA 3.3 Part 2 §10.3.1.1:
  there are no trust rules to allow it); the target is stateless, every
  request brings its own context. Every reply of the request carries the SAS answer
  (`CompleteEstablishContext` or `ContextError`).
- `EstablishTrustInClient` (cert) + `EstablishTrustInTarget` (cert): the
  TLS layer of SSLIOP; GSSUP username/password as fallback
  (`gssup.target_name`). A request with neither a verified client
  certificate nor GSSUP credentials is `NO_PERMISSION`, so is one whose
  context CSIv2 rejects.
- the IOR carries `TAG_CSI_SEC_MECH_LIST`: a mechanism over TLS
  (`TAG_TLS_SEC_TRANS` with the SSLIOP address) with SSLIOP, and one over
  plain IIOP (`TAG_NULL_TAG`) unless `ssliop.required`; GSSUP as the
  authentication layer, supported over TLS with a CA and required where no
  certificate can establish the trust; identity assertion as
  `target_supports` names.

Subject resolution: from the TLS cert subject DN or the GSSUP username
(`gssup:<username>` for the ACL, §7.3).

### §7.3 ACL

Subject = subject DN of the client certificate per RFC 4514 (`CN=trader,O=…`);
a rule may instead name the fingerprint `mtls:<SHA-256, hex>`.
With CSIv2 the GSSUP username is a subject too, as `gssup:<username>`.
With plain IIOP and no CSIv2 there is no subject. A rule with a matching subject (or
`*`) must list the `repo_id` in `allow_invoke` (prefix with `*`); without
a matching rule, `default_deny` decides. Refused: `NO_PERMISSION`.

## §8 Operations + observability

### §8.1 Logging

JSON log on stderr, one line per event, filtered by `log_level`.
Fields: `timestamp`, `level`, `event`, `peer`, `giop_version`, `request_id`,
`operation`, `repo_id`, `bytes`, `reply_status`, `latency_us`.

Every line carries `timestamp` (RFC 3339, UTC, milliseconds), `level` and
`event`. The end of every GIOP exchange is the event `exchange` (level
info) with `peer`, `giop_version`, `request_id`, `operation`, `repo_id`
(of a mapped object), `bytes` (of the request body), `reply_status`
(`NO_EXCEPTION`, `USER_EXCEPTION`, `SYSTEM_EXCEPTION`,
`LOCATION_FORWARD`, `NONE` for a oneway), `exception` (the exception's
name), `latency_us` and, for a tenant's participant, `tenant`; numbers are
JSON numbers.

### §8.2 Prometheus metrics

```
zerodds_corba_bridge_requests_total            counter{operation, status}
zerodds_corba_bridge_replies_sent_total        counter{operation}
zerodds_corba_bridge_pending_requests          gauge{operation}
zerodds_corba_bridge_request_latency_seconds   histogram{operation}
zerodds_corba_bridge_timeouts_total            counter{operation}
zerodds_corba_bridge_dds_samples_received      counter{dds_topic}
zerodds_corba_bridge_dds_samples_published     counter{dds_topic}
zerodds_corba_bridge_locate_requests_total     counter
zerodds_corba_bridge_csiv2_failures_total      counter{reason}
zerodds_corba_bridge_giop_message_errors_total counter
```

Built are the families above, plus the generic ones of all bridges
(`zerodds_corba_frames_*`, `_bytes_*`, `_connections_*`, `_dds_samples_*`,
`_errors_total`). `status` is `no_exception`, `user_exception` or
`system_exception` (TIMEOUT and rejections are counted there as well).
`reason` of `csiv2_failures_total` is `missing` (neither a client
certificate nor a SAS context), or the major status of the `ContextError`:
`invalid_evidence`, `invalid_mechanism`, `conflicting_evidence`,
`no_context`, `other`. The label values come from the configuration, not
from the wire; a request to an unknown object is counted under no
operation. A tenant's `operation` and `dds_topic` carry its name first:
`<tenant>/<operation>`.

### §8.3 OTLP spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → spans per GIOP exchange (`giop.request`
+ `dds.publish` + `dds.subscribe` + `giop.reply` as children).

The root `giop.request` (`SpanKind::Server`, from the request to its
reply) has the children `dds.publish` (the request sample written),
`dds.subscribe` (the reply sample taken) and `giop.reply` (the reply
written) where the exchange had them; a refused request has `giop.reply`
only. Attributes: `rpc.system` `corba`, `rpc.method`, `net.peer`,
`giop.version`, `giop.reply_status`, `corba.repo_id`, `corba.exception`,
`dds.topic`, `zerodds.tenant`. The exporter flushes every 5 s and at the
stop.

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation.
2. TLS cert load if SSLIOP.
3. DCPS DomainParticipant init, one per tenant beside the one of `domain`
   (§1 L6); a participant that does not start is exit 3.
4. Per mapping: topic auto-generation, register reader+writer.
5. IIOP bind 6833 + SSLIOP bind 6834 (optional).
6. NameService resolve + rebind of all IORs (if `publish_to_naming`).
7. Install SIGHUP/SIGTERM/SIGINT handlers.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- NameService unbind of all IORs (with `publish_to_naming`), first, so
  that no client resolves the bridge during the drain.
- No new connections; a request during the drain is answered with
  `TRANSIENT` (completed NO).
- Open requests get their reply or their `TIMEOUT` (`timeout_ms`);
  whatever is still open after 30 s gets `TRANSIENT` (completed MAYBE).
- Then `CloseConnection` to every connection, in its GIOP version, and
  the connection is closed (GIOP §15.4.7: after `CloseConnection`
  no reply follows, therefore only after the replies).
- Every participant drains at the same time.
- Exit 0.

SIGHUP → config reload (TLS cert + ACL hot update; mapping changes
need a restart). Built: the daemon re-reads the same layers (files,
`--set`, flags); ACL and SSLIOP material apply from then on, the material for
connections that are accepted afterwards, for every participant. Changed
mappings, binds, `ssliop.enabled`, `domain` or `tenants` are reported as
`reload_needs_restart`. A
configuration that does not load, or material that does not load, changes nothing
(`reload_refused` with a reason).

## §10 Cross-vendor

The daemon is a normal RTPS peer. CORBA side tested against:
- TAO (ACE+TAO)
- JacORB
- omniORB

Ice (ZeroC) is no peer: the Ice protocol is its own and not compatible
with GIOP on the wire, and Ice has no IIOP mode (Ice manual, "The Ice
Protocol", comparison with IIOP).

Measured against omniORB 4.3.3 (Linux test host) in
`crates/corba-dds-bridge/tests/omniorb_e2e.rs`: an omniORB client calls a
DDS backend via the IOR of the bridge (`add` → 42), `TIMEOUT` without a backend,
`BAD_OPERATION`, an `echo` of 200,000 bytes fragmented in both
directions; three DDS samples reach an omniORB servant as oneway.
`catior` reads the IOR (`tests/cross_vendor.rs`). SSLIOP with client certificate and ACL:
the allowed certificate passes, another one from the same CA gets
`NO_PERMISSION`. JacORB 3.9 (`tests/jacorb_e2e.rs`) and TAO
(`tests/tao_e2e.rs`): calls to a DDS backend, oneways to their servants,
the NameService bind; JacORB with GSSUP credentials through CSIv2.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-corba-bridged`
- Config default: `/etc/zerodds/corba-bridged.yaml`
- Systemd unit: `zerodds-corba-bridged.service`
- launchd plist: `org.zerodds.corba-bridged.plist`
- Win service: `ZeroDDSCORBABridge`
- Docker: `zerodds/corba-bridged:1.0`
- IOR backup: `corba.ior.ior_file`, by default in the example
  `/var/lib/zerodds/corba-bridge.ior` (the unit may write `/var/lib/zerodds`)

Manual: `man 1 zerodds-corba-bridged` + `man 5 zerodds-corba-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `giop_codec`, `iiop_transport`, `ior`,
`object_key`, `csiv2`, `dds_pump`, `request_correlator`)
≥ 5 tests in `crates/corba-giop/`, `crates/corba-iiop/`,
`crates/corba-dds-bridge/`. In the daemon (`src/bridge_daemon/`) they are
`config.rs`, `framing.rs` (the GIOP frames of the daemon beside the codec of
`corba-giop`), `ior.rs`, `object_key.rs`, `csiv2_wire.rs`, `pump.rs`
(`dds_pump`), `correlator.rs` (`request_correlator`), and `server.rs`,
`notify.rs`, `log.rs`, `trace.rs`, `typegen.rs`.

### §12.2 Integration tests

- `crates/corba-dds-bridge/tests/omniorb_e2e.rs` (omniORB instead of TAO; the
  cases from §10).
- `crates/corba-dds-bridge/tests/bridge_e2e.rs`: the binary, unknown
  object → `OBJECT_NOT_EXIST`, `--dump-iors`, the exit codes, the drain,
  SIGHUP, the metrics, the log line and the spans, a tenant bound by its
  client certificate, the forward of SSLIOP-only.

### §12.3 Multi-vendor

A CORBA client and a DDS backend of other vendors around the bridge:
`crates/corba-dds-bridge/tests/omniorb_e2e.rs` lets an omniORB client call
`Calc::add` and a Cyclone DDS program answer it on the DDS side
(`tests/cyclone/calc_backend.c`, its types by the layout of §4.3 in
`tests/cyclone/CalcDds.idl`, written in XCDR1). A second Cyclone DDS program
(`tests/cyclone/div_backend.c`) is built on the IDL the bridge generated
from `tests/omniorb/Div.idl` (§5.2) through idlc and answers an omniORB
client with a result, a user exception with members and one without. TAO and JacORB clients and
servants against ZeroDDS backends: `tests/tao_e2e.rs`, `tests/jacorb_e2e.rs`
(§10). They run in the weekly jobs `integration-bridges-vendor-corba` and
`vendor-live-corba-tao`.

## §13 Cross-references

- Library: `crates/corba-dds-bridge/`, `crates/corba-iiop/`,
  `crates/corba-giop/`, `crates/idl-rust/` (IDL codegen).
- OMG specs: GIOP/IIOP (formal/2012-11-14), CSIv2 (formal/2008-01-01),
  SSLIOP (formal/2008-01-01).
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3 (CDR body).
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-grpc-bridge-1.0` (RPC equivalent),
  `zerodds-amqp-bridge-daemon-1.0`.

## §14 Versioning

`1.0` initial. Patch for bug fixes, minor for additive mapping
configuration, major for wire protocol changes.
