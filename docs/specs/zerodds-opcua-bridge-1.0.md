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
per-session check this daemon has had. `Server::set_write_commit` hands a
Write the AddressSpace accepted to a second closure, whose status is the
status of the Write; the daemon writes into DDS in it (§6.2).

### §4.3 Part 14 UADP

One `Publisher`/`Subscriber` pair per configured `opcua.pubsub.connections[]`
entry, over `UdpTransport` (unicast/multicast UDP). Message security
(`SecurityGroup`/SKS, feature `pubsub-security`) is wired via
`opcua.pubsub.connections[].security` (`policy`, `security_group_id`,
`key_source`). The keys come from one of four sources (Part 14 §5.4.5):

- `static`: one key (`token_id`, `signing_key`/`encrypting_key`/`key_nonce`
  as hex) that does not rotate; a key whose length disagrees with its
  policy is refused at load time.
- `local`: the daemon's own SecurityGroup (below).
- `sks`: pulled with `GetSecurityKeys` (Part 14 §8.3.2) from a Security Key
  Service over a SecureChannel that encrypts, with `StartingTokenId` 0;
  pulled again at half the key lifetime, after a failure every 5 s, and at
  once when a received message names a token there is no key for. The
  next key becomes current after `TimeToNextKey` (counted from the
  answer), each further one a `KeyLifetime` later; a fetched list is
  matched with the keys at hand by its `FirstTokenId`.
- `push`: pushed with `SetSecurityKeys` (Part 14 §9.1.3.3, `i=17364`) by
  the session of `push_subject` over an encrypted channel.

The `DataSetMetaData` a subscriber reads a DataSet by (Part 14 §6.2.3.2)
describes each field with the built-in type it is transferred as (an
enumeration Int32, a structure, a union, a bitmask, a `map` and a `Decimal`
ExtensionObject, a collection of collections Variant), its DataType,
ValueRank and ArrayDimensions (§5.3) and a `DataSetFieldId` of its own, the
same for the same field of the same DataSet at every start (two 64-bit
FNV-1a hashes of the DataSet's and the field's names); its `namespaces` are
the server's ApplicationUri and the daemon's namespace URI (indices 1 and
2), its `structureDataTypes` and `enumDataTypes` the descriptions of the
DataTypes the fields use, with `KeyValuePair` for a `map` (§6.2.3.2.2:
DataTypes of namespace 0 that are no built-in types are described too). A
field that names no part of the type refuses the connection at start.

Until the first keys are there a connection publishes and reads nothing
(the PreOperational state of §5.4.5.3). A subscriber keeps the previous
key besides the current and the future ones.

With the build features `crypto` and `pubsub-security` the daemon is a
Security Key Service for the SecurityGroups of
`opcua.server.security_groups` (Part 14 §8): it makes the keys from the
operating system's random source, the first with token 1, keeps
`max_future_key_count` future and `max_past_key_count` past keys, and
rotates on the schedule of `key_lifetime_ms`. Its server has the
`PublishSubscribe` Object (`i=14443`, a component of the Server Object)
with `GetSecurityKeys` (`i=15215`) and `GetSecurityGroup` (`i=15440`), the
`SecurityGroups` folder (`i=15443`) and an Object of `SecurityGroupType`
for each group with its five Properties and the Methods `InvalidateKeys`
and `ForceKeyRotation`; the Methods have their `InputArguments` and
`OutputArguments`. `GetSecurityKeys` answers `Bad_SecurityModeInsufficient`
over a channel that does not encrypt, `Bad_UserAccessDenied` to a user not
in `key_subjects`, `Bad_NotFound` for an unknown group, and returns at most
`max_future_key_count` + 1 keys. `InvalidateKeys` and `ForceKeyRotation`
need a signed channel and a user in `admin_subjects`.

The `SecurityGroups` folder is a `SecurityGroupFolderType` (§8.5) with
`AddSecurityGroup` (`i=15444`), `RemoveSecurityGroup` (`i=15447`),
`AddSecurityGroupFolder` (`i=25434`), `RemoveSecurityGroupFolder`
(`i=25437`) and `SupportedSecurityPolicyUris` (`i=25439`: AES-256-CTR, the
default, AES-128-CTR, AES-256-GCM); a folder added has the four Methods
too (`SecurityGroupFolder:<path>/<Method>`). `AddSecurityGroup` needs a
signed channel; a `KeyLifetime` of 0 is one hour, others are held to 1 s
to 30 days, a `MaxFutureKeyCount` of 0 is 3, both counts are held to 64,
an empty `SecurityPolicyUri` is the default and an unknown one
`Bad_InvalidArgument`; the group asked for again is returned with
`Good_DataIgnored`, another of its name is `Bad_NodeIdExists`.
`RemoveSecurityGroup` (signed) answers `Bad_NodeIdUnknown` for an unknown
node and `Bad_NodeIdInvalid` for one that is no SecurityGroup of the
folder; `AddSecurityGroupFolder` answers `Bad_BrowseNameDuplicated` for a
name the folder has, and `RemoveSecurityGroupFolder` removes the folder,
its folders and their groups. Groups and folders added at run time are not
kept across a restart.

The users of the roles of §8.8 are `opcua.server.roles`:
`security_key_server_admin` may call the folder Methods and
`InvalidateKeys`/`ForceKeyRotation` of every group (a user here makes the
daemon an SKS without configured groups), `security_key_server_access` may
call `GetSecurityKeys` for every group, `security_key_server_push` may
call `SetSecurityKeys` for every connection with `key_source: push`. The
per-group `key_subjects`/`admin_subjects` and the per-connection
`push_subject` are custom roles of one group or connection. The role
Objects of OPC 10000-18 are not in the AddressSpace, and
`SessionlessInvoke` (§5.4.5.4) is not built.

The SKS pushes keys to targets (§8.6, §8.7). `KeyPushTargets` (`i=25440`,
`PubSubKeyPushTargetFolderType`) has `AddPushTarget` (`i=25441`),
`RemovePushTarget` (`i=25444`), `AddPushTargetFolder` (`i=25446`) and
`RemovePushTargetFolder` (`i=25449`); a folder added has them too. A target
(`KeyPushTarget:<ApplicationUri>`, `PubSubKeyPushTargetType`) has the
eight Properties of Table 221 and `ConnectSecurityGroups`,
`DisconnectSecurityGroups`, `TriggerKeyUpdate`; a connected group is the
target of a `HasPushedSecurityGroup` reference (`i=25345`). The methods
need a user of `security_key_server_admin`, the target and group methods
a signed channel. `AddPushTarget` refuses a `SecurityPolicyUri` that is no
SecureChannel policy of the daemon, a `RequestedKeyCount` below 3, a
`RetryInterval` that is not positive and a `UserTokenType` other than
Anonymous (the SKS has no secret for another token to give; §8.6.1 makes
Anonymous the default, authorized by the application identity of the
SKS). A thread pushes: when a group is connected, on `TriggerKeyUpdate`,
then at half the shortest `KeyLifetime` of the target's groups, after an
error after `RetryInterval`. A push asks the target's endpoint for its
endpoints, takes the SignAndEncrypt one of the policy, fails when its
ApplicationUri is not the target's, reads the public key from the
endpoint's certificate, opens a SignAndEncrypt SecureChannel with the
certificate and the private key of the daemon's own server
(`opcua.server.certificate_der`, `private_key_pem`) and an anonymous
session, and calls `SetSecurityKeys` per group with the current key and
`RequestedKeyCount` − 1 future keys (as many as the group keeps). The time
goes into `LastPushExecutionTime` or `LastPushErrorTime`. Targets added at
run time are not kept across a restart.

## §5 Topic mapping

### §5.1 Types: TypeObject/TypeLookup → `SerializedTypeSupport`

A topic's DDS type is never a compiled Rust type here. `opcua.idl` files
are compiled (`zerodds-idl-compose`) into a model; a `DynamicType` for the
type a config entry names is turned into a `CompleteTypeObject`
(`zerodds_types::from_ast::complete_type_object`), with the
`CompleteTypeObject`s of the types it refers to by hash (a structure, a
union or an enumeration as a member, `dependency_type_objects`), and then a
`zerodds_dcps::serialized_type::SerializedTypeSupport` — the same
reflective decoder/encoder `crates/xrce-agent`'s daemon and
`crates/routing-service` use for a topic with no compiled type. This is the
"types via TypeObject/TypeLookup" path.

A type that no `opcua.idl` file declares comes from the remote endpoints
of the topic, over TypeLookup (DDS-XTypes 1.3 §7.6.3.3). Such a topic — a
`dds_to_ua_connection`, a `ua_to_dds_connection`, a node of
`opcua.client.servers[]` or a PubSub publisher or subscriber — waits; the
daemon says so at start (`warn`) and once a minute while it waits
(`info`). Every 500 ms it looks at the remote endpoints of the topic
(publications and subscriptions alike): one that announces a complete
`TypeIdentifier` (the complete half of `PID_TYPE_INFORMATION`, XTypes 1.3
§7.6.3.2.2; the minimal half has no member names) is asked for the type
with `getTypes` and for its dependencies with `getTypeDependencies`, again
after 5 s without the type. Once the runtime's TypeLookup registry holds
the type and every type it depends on, the topic is built as it would have
been at start: its AddressSpace nodes, its DDS reader or writer and its
job; the endpoints of the daemon name the type as the gateway file does.
A remote endpoint that announces no type gives none.

The endpoints of the daemon announce their own type: the
`CompleteTypeObject`s of the type go to the runtime's TypeLookup registry
before the endpoint is announced, and the endpoint announces the type's
`TypeIdentifier`, so that a peer that does not know the type can ask the
daemon for it.

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

The server has the `Server` Object (`i=2253`, Part 5 §8.3.2), organized
by `Objects`, with its `ServerArray` (`i=2254`) and `NamespaceArray`
(`i=2255`): the OPC UA namespace, the ApplicationUri
`urn:zerodds:opcua-server`, and at index 2 the daemon's namespace
`urn:zerodds:opcua-bridge`, the one of every NodeId and DataType it
creates. The other components of `ServerType` (Part 5 §6.3.1) are not
built.

A `dds_to_ua_connection` leaf's `AccessLevel` is `CurrentRead` only, so a
foreign client's `Write` against it fails `Bad_NotWritable` rather than
succeeding with no effect. A `ua_to_dds_connection` leaf is `CurrentRead |
CurrentWrite`.

### §5.3 Type-shape coverage

Primitives, strings (narrow and wide — `wchar`/`wstring` map to
`UInt16`/`String`, mirroring `dds_to_ua::walker`'s type-level mapping),
enumerations, bitmasks, nested structs, unions, `map<K,V>` (one leaf per
entry, path-named `<field>.<key>`; a map-of-struct decomposes further, same
as any nested aggregate), and arrays/sequences of any of the above are
mapped, both directions.

**DataTypes (Gateway 1.0 §9.2).** Every structure, union, enumeration and
bitmask a topic's type uses is a DataType node: `<Name>DataType`, a subtype
of `Structure` (`i=22`), `Union` (`i=12756`), `Enumeration` (`i=29`) or
`OptionSet` (`i=12755`), with its `DataTypeDefinition` (OPC 10000-3
§5.8.3): a `StructureDefinition` for a structure (a field per member in the
order of the members; `StructureWithOptionalFields` when a member is
`@optional`) and for a union (a field per case, `StructureType` `Union`),
an `EnumDefinition` for an enumeration and for a bitmask (for a bitmask the
`Value` of a field is the number of its bit, §8.52). A structured DataType
has its `Default Binary` encoding node (`HasEncoding`, type
`DataTypeEncodingType`), whose NodeId is the TypeId of its
ExtensionObjects. An enumeration has the `EnumValues` Property
(`EnumValueType[]`, Table 9.7), a bitmask the `OptionSetValues` Property, an
entry per position of its bound with the name of the flag there or
`UndefinedPosition_<position>` (Table 9.11; OPC 10000-3 1.05.06 Table 16
asks for a null text at an undefined position, the Gateway for this name,
and the daemon follows the Gateway). A structure also has
`<Name>VariableType` (Table 9.16), a subtype of `BaseDataVariableType` with
a component Variable per member, `ModellingRule` `Optional` for an
`@optional` member and `Mandatory` for every other. The NodeIds are
`DataType:<scoped name>`, `DataType:<scoped name>/DefaultBinary`,
`VariableType:<scoped name>` in the daemon's namespace; two different types
of one scoped name refuse the topic that brings the second (its values would
carry the first one's TypeId). The namespace-0 types the DataTypes are
subtypes of (`BaseDataType`, `Structure`, `Enumeration`, `OptionSet`,
`Union`, `BaseVariableType`, `BaseDataVariableType`) are nodes of the
AddressSpace, so that a client that loads the DataTypeDefinitions of a
server by browsing `HasSubtype` from `Structure` finds them.

Every Variable of a topic has the `DataType`, `ValueRank` and
`ArrayDimensions` of its DDS type (Gateway Table 9.2 for the primitive
types, the generated DataType for a structure, union, enumeration or
bitmask, `ValueRank` the number of dimensions of an array, `1` with
`ArrayDimensions` `[bound]` or `[0]` for a sequence) and a `HasTypeDefinition`
reference: `<Name>VariableType` for a structure, `BaseDataVariableType` for
every other (Tables 9.8, 9.12, 9.21). A `map` member is a Variable, not the
Object of Gateway Table 9.40: OPC 10000-3 §7.7 lets only an Object or an
ObjectType have an Object as a component, and a map member is a component
of a structure Variable. Its DataType is `KeyValuePair` (`i=14533`), an
array: an entry's key is a QualifiedName of the text the entry's Variable is
named by (Table 9.41), its value a Variant. A structure field of a map type
is the same array.

**Values.** A leaf's Value is its member's value in the built-in type of its
DataType; an enumeration is its Int32 value (Table 9.8). A bitmask is its
`OptionSet` (Table 9.12): `Value` and `ValidBits`, each as many octets as
the bound needs, bit `n` in bit `n % 8` (from the least significant) of
octet `n / 8`; a bit is valid when the bitmask names a flag at it. A Write
of a bitmask sets the valid bits written and keeps the others ("new value =
(value & validBits) | (current value & ~validBits)", OPC 10000-3 §8.40); a
valid bit at a position without a flag is refused. The Gateway's "length
equal to the bound" of the two ByteStrings is read as the number of bits;
§8.40 sizes them in octets.

The Value of an aggregate Variable is the whole of what is under it
(Gateway 1.0 §9.2.4.1.1, Table 9.19): a structure, a union or a bitset is
an ExtensionObject of its DataType in its `Default Binary` encoding (OPC
10000-6 §5.2.6: the fields in the order of the definition; §5.2.7: first a
UInt32 mask with a bit per optional field, set when the sample has it;
§5.2.8: the UInt32 switch, the position of the active case counted from 1,
0 when no case is active, then the case); an array or a sequence of
structures or unions is the array of their ExtensionObjects, each element
Variable its own; a `map` is the array of its `KeyValuePair`s. Inside a
structure an array of several dimensions is the inline matrix of §5.2.5
(Table 28: the dimensions, then the values); the one-dimensional form an
encoder before OPC 10000-6 1.05 writes (the count of all elements, then the
values) is read too, where the count differs from the number of
dimensions. A union's discriminator is the leaf
`SwitchField` (the discriminator's value in its own type); the case a
sample does not hold, like an `@optional` member it does not have, keeps
its nodes and reads `Bad_NoData`.

A Write of an aggregate Variable of a `ua_to_dds_connection` replaces that
part of the sample with the value written (an ExtensionObject of another
DataType, or one that does not decode, is refused); a Write of a leaf
replaces the member. A Write to a case of a union that is not active makes
it the active one, with the case's first label as the discriminator (for
the default case, the smallest non-negative value no other case names); a
Write of `SwitchField` selects the case of the discriminator written. A
Write to a member of an `@optional` member the sample does not have makes
it present. The sample written to DDS is the one written last with the
change in it; every Variable of the topic then shows that sample.

`long double` is a `ByteString` of 16 octets (Gateway 1.0 §9.2.1: "to
store the Float128 value, the length of the equivalent ByteString shall be
16"), the binary128 value least significant octet first, the order of the
numbers in the OPC UA binary encoding; the Gateway names no order. A
`bitset` is the structure of its named bitfields, one leaf each
(`<field>.<bitfield>`), a number of the bitfield's holder type: the one
the IDL names, or a Boolean for one bit, a Byte up to 8, a UInt16, UInt32
or UInt64 above; its DataType is a `Structure` of those fields. Gateway 1.0
has no clause for a bitset. Both directions.

An array or a sequence of structures or unions has one Variable per
element (Gateway 1.0 §9.2.5.1.2.3, §9.2.5.2.2.3), named
`<TypeName>_<index>` (for an array of several dimensions the position in
each, `<TypeName>_1_2_3`), an ordered component (`HasOrderedComponent`) of
the collection's Variable, with the element's members under it. The
elements of an array are there from the start; a sequence gets an
element's Variable when a sample brings the element, and an element a
later sample no longer has keeps its node and reads `Bad_NoData`. A Write
to a member of an element of structures changes that member; it adds no
element. A Variant written to an array or a sequence of scalars replaces
the collection; an array takes as many values as it has elements, an array
of several dimensions in the order of the serialized elements, the last
dimension fastest.

An array or a sequence whose elements are collections (arrays, sequences,
`map`s) has one Variable per element (Gateway 1.0 §9.2.5.2.2.5), named
after the collection, `<SequenceName>_<index>` (the position in each
dimension for an array of several), an ordered component of the
collection's Variable; the element Variable is what its own type makes it
(an array leaf, a `map` with its entries, a collection of structures with
its elements). The Gateway's Object for the outer collection is a
Variable here, for the reason a `map` is (OPC 10000-3 §7.7). OPC UA has no
array of arrays: the outer Variable's DataType is `BaseDataType` with the
collection's ValueRank, it has no Value of its own (the Variant model of
the daemon holds no Variant in a Variant), and in a structure field each
element is a Variant holding the inner collection (OPC 10000-6 §5.2.6,
a field of an abstract DataType).

`fixed<P,S>` is an OPC UA `Decimal` (OPC 10000-6 §5.1.10): an
ExtensionObject of the Decimal DataType (`i=50`) whose body is the Int16
scale `S` and the unscaled value in the fewest two's complement octets,
least significant first. A Decimal written to such a leaf is rescaled to
`S`; one with more fraction digits than `S` holds or more than `P` digits
is refused. Gateway 1.0 has no clause for `fixed`.

A `map` key that a DDS sample brings and that has no node yet gets its
nodes when the sample is applied: a Variable named by the key, a component
of the map member's node, with the NodeId
`Topic:<name>/Value/<member>.<key>`, and for a struct value the members
under it. The node stays when a later sample no longer has the key; its
Value is then the last one. A topic gets nodes for at most 4096 entries
while the daemon runs and for keys of at most 256 octets; a key past
either bound is reported as an unmatched path. In a `PublishedDataSet`
the key is a field appended at the end (so is any other path the type's
default sample did not have: a member of another union branch, an
optional member), the DataSetWriter advertises a new `MinorVersion`
(Part 14 §6.2.3.2.5) and sends a key frame next; a DataSet grows to at
most 4096 fields. A client cannot introduce a key through a Write: OPC UA
writes to nodes that exist, and the daemon does not serve AddNodes.

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
is decoded, flattened to named leaf values and the values of its aggregate
Variables (§5.3), and applied to the AddressSpace via
`AddressSpace::set_value`. Best-effort delivery — a sample that never
matches the reader (e.g. QoS-incompatible) never reaches a value, same as
any DDS DataReader.

### §6.2 OPC UA → DDS (§8)

Two independent mechanisms:

1. **A third-party client writes to this daemon's own server.** The Write
   first passes `Server::set_write_hook`'s installed closure (§4.2), which
   enforces `acl.rules[].subject` against the writing session's real
   identity (§7.2) — a denial is `Bad_UserAccessDenied`, synchronously,
   before the AddressSpace is ever touched. A write the hook allows and
   the AddressSpace accepts is written into DDS before the client is
   answered (`Server::set_write_commit`): the changed Variables of the
   `ua_to_dds_connection`, leaves and aggregate Variables, are put into the
   sample written last (§5.3), which is encoded and written, and the status
   of the Write is the outcome. A value that does not encode for the topic's type
   or that DDS does not take is answered `Bad_CommunicationError`, logged
   with its topic, and taken back out of the AddressSpace, so that a Read
   shows the value before it. A value the daemon itself puts into such a
   leaf is still found by the pump loop, which compares every leaf with its
   last-seen value every 100 ms.
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

`log_level` (`trace`/`debug`/`info`/`warn`/`error`) to stderr: a line is
written when its level reaches `log_level`, as
`[zerodds-opcua-bridged] <level>: <message>`. Every skip (`warn`), decode
failure and DDS-write failure (`error`) is logged by name (topic/path),
never silent.

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
the scenario `bridge-opcua` of the integration pillar
(`tests/integration/tests/bridges.rs::opcua_bridge_against_open62541`, job
`integration-bridges`; see `crates/opcua-bridge/tests/foreign/README.md`)
drives a real running `zerodds-opcua-bridged` from a tiny C
client built against `libopen62541`'s own public API — Browse the
`Objects` folder, Read a `dds_to_ua_connection` leaf, Write a
`ua_to_dds_connection` leaf and confirm the write reached DDS. A generic
OPC-UA client (UaExpert or any
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

The scenario `bridge-opcua` of the integration pillar (§10) drives a real
running daemon from a tiny C client against `libopen62541` in the job
`integration-bridges`. `crates/opcua-server/proofs/
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
open62541 daemon-level interop measurement (§12.3/§10). Still open: the
SecurityGroups added at run time and KeyPushTargets of the SKS
(§4.3). Closed since: the DataTypes, VariableTypes and DataType attributes of Gateway 1.0
§9.2 and the Value of the aggregate Variables, unions, `@optional` members
arrays of several dimensions and collections of collections (§5.3), the members a Write does not name
keep the value written last (§6.2), a type with a structure, a union or an
enumeration as a member gets its type support (§5.1), the answer to an authorized Write is the outcome of
the write into DDS (§6.2), `log_level` filters the log (§8.1), a map key a
sample brings gets its node and its DataSet field (§5.3), a type in no IDL
file comes over TypeLookup (§5.1), and the PubSub keys come from a Security
Key Service, the daemon's own or another, and rotate (§4.3).
