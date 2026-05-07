# DDS in 5 minutes

DDS is a publish-subscribe data-distribution standard. Four nouns:

## Domain

A logical isolation boundary, identified by a small integer
(typically `0`). Two participants on different domains never see
each other, even on the same network.

```
Domain 0:  ┌─────────┐    ┌─────────┐
           │ Robot A │ ↔  │ Robot B │
           └─────────┘    └─────────┘

Domain 7:  ┌─────────┐    ┌─────────┐
           │ Sim env │ ↔  │ Replay  │
           └─────────┘    └─────────┘
```

## Topic

A typed named channel. Topics carry a single IDL type; subscribers
match on `(topic_name, type_name)` and the QoS contract.

```
Topic "Telemetry" / Type "Robot::Telemetry"
   ┌──────────┐                          ┌──────────┐
   │ Writer A │ ──── samples ────▶       │ Reader X │
   └──────────┘                          └──────────┘
```

## Publisher / DataWriter

`Publisher` is a factory + scope; `DataWriter` is the per-topic
endpoint that calls `write(sample)`. One publisher can hold many
data-writers across different topics.

## Subscriber / DataReader

Symmetric to Publisher: `Subscriber` is the factory, `DataReader`
is the per-topic endpoint that calls `take()` to pull arrived
samples.

## QoS — the contract

Publishers offer QoS, subscribers request QoS. Two endpoints match
only if the offered ⊇ requested.

The most-used policies:

| Policy | Default | Meaning |
|---|---|---|
| `Reliability` | `BestEffort` | `Reliable` retries until ack; `BestEffort` ships once |
| `Durability` | `Volatile` | `TransientLocal` lets late-joining readers replay |
| `History` | `KeepLast(1)` | `KeepAll` retains until cache cap; `KeepLast(N)` evicts oldest |
| `Deadline` | infinite | Period within which a sample must arrive |
| `Liveliness` | infinite | Heartbeat interval to assert "writer is alive" |
| `Ownership` | `Shared` | `Exclusive` → only highest-strength writer wins per instance |

Full reference: [03 Configuration → QoS](../03-configuration/qos-policies.md).

## Discovery

Endpoints find each other automatically via:

1. **SPDP** — Simple Participant Discovery Protocol on UDP
   multicast (default `239.255.0.1:7400 + 250×domain_id`).
2. **SEDP** — Simple Endpoint Discovery Protocol over reliable
   built-in topics, after participants have found each other.

This is why your hello-world publisher and subscriber find each
other without any explicit address configuration.

## Wire format

ZeroDDS speaks **DDSI-RTPS 2.5** on the wire, byte-identical to
Cyclone DDS, FastDDS, RTI Connext, and OpenSplice. Sample payloads
are encoded in **CDR / XCDR2** per OMG-XTypes 1.3.

## Next

→ [First publisher / subscriber](first-publisher.md) — let's see
this in code.
