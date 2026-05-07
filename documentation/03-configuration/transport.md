# Transports

ZeroDDS ships five transports. Pick by topology + latency / loss
profile.

| Transport | Crate | When |
|---|---|---|
| UDP | `zerodds-transport-udp` | Default. Cross-host, multicast discovery, low overhead. |
| TCP | `zerodds-transport-tcp` | Cross-network with NAT, when multicast is unavailable. Higher latency. |
| Shared Memory | `zerodds-transport-shm` | Same-host, zero-copy, sub-µs RTT. POSIX SHM segments per writer-reader pair. |
| Unix Domain Sockets | `zerodds-transport-uds` | Same-host, secure (filesystem ACLs), no kernel firewall path. |
| TSN | `zerodds-transport-tsn` | Time-Sensitive Networking — IEEE 802.1Qbv schedule integration. |

## UDP (default)

The discovery path is hard-wired UDP:

- SPDP multicast: `239.255.0.1` port `7400 + 250 × domain_id`.
- SPDP unicast: ephemeral port assigned by the kernel.
- User data unicast: ephemeral port.

The UDP transport handles MTU-based fragmentation in cooperation
with `rtps::DataFragSubmessage`. Default fragment size 1344 bytes
(1400 MTU − 20 RTPS header − ~32 byte submessage overhead) — tune
via `ReliableWriterConfig.fragment_size`.

## TCP

Use when:

- Multicast is filtered (cloud, enterprise WAN, multi-VLAN
  without IGMP-snooping).
- NAT traversal required.
- TLS-wrapping desired (deploy behind a reverse proxy or future
  built-in TLS support).

TCP loses some DDS properties — discovery latency goes up, no
multicast-fanout — but works everywhere.

## Shared Memory

`zerodds-transport-shm` is a POSIX-SHM ring per writer-reader pair.
Lock-free SpSc protocol via AcqRel atomics on `head`/`tail`.

Pros:
- Zero-copy on the receiver side (same process or shared mmap).
- Sub-µs latency on tight loops.
- No kernel network-stack overhead.

Cons:
- Same host only.
- Per-pair segment ⇒ N segments for N readers (default 1 MiB
  each — 100 readers = 100 MiB).
- Crash-recovery uses `shutdown` flag; abandoned mmaps are
  cleaned by the OS on process exit.

## Unix Domain Sockets

`zerodds-transport-uds` — like UDP but on a UNIX socket. Pros: no
network stack, FS ACL for security, no port collisions. Cons:
same-host only, slightly higher overhead than SHM.

## TSN

`zerodds-transport-tsn` — time-aware shaper integration for
deterministic Ethernet (IEEE 802.1Qbv). Requires kernel + NIC
support (`SO_TXTIME`, hardware queues). Production-ready in
preempt_rt + isolcpus deployments.

## Mixing transports

A single `DcpsRuntime` can speak UDP for discovery and SHM for
same-host data — discovery announces both locators, peers pick
the cheapest. The locator-list logic lives in
`zerodds-discovery::participant_data` (`default_unicast_locators`,
`default_multicast_locators`, plus per-endpoint locator lists).

## Transport selection in code

The default `DcpsRuntime` opens UDP only. SHM and UDS are opt-in
via `RuntimeConfig.interface_bindings` and the per-endpoint
`InterfaceBindingSpec`.

For a typed deep-dive see `crates/transport/src/lib.rs` — the
`Transport` trait and `Locator` types are the contract.
