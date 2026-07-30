# RuntimeConfig

`zerodds_dcps::runtime::RuntimeConfig` controls everything that lives
above the DCPS API.

```rust
use std::time::Duration;
use std::net::Ipv4Addr;
use zerodds_dcps::runtime::RuntimeConfig;

let cfg = RuntimeConfig {
    tick_period: Duration::from_millis(50),
    spdp_period: Duration::from_secs(5),
    spdp_multicast_group: Ipv4Addr::new(239, 255, 0, 1),
    multicast_interface: Ipv4Addr::UNSPECIFIED,
    announce_secure_endpoints: false,
    wlp_period: Duration::ZERO,                  // = lease/3
    participant_lease_duration: Duration::from_secs(100),
    observability: zerodds_foundation::observability::null_sink(),
    // … security fields when feature = "security"
    ..Default::default()
};
```

> ▶ Runnable example: [`runtime-config`](https://github.com/zero-objects/zero-dds-snippets/tree/master/runtime-config)
> (starts a real `DcpsRuntime` with this exact field list; also covers the
> three "Common combinations" recipes below).

## Field reference

### `tick_period`

How often the event-loop ticks: liveliness checks, deadline checks,
lifespan eviction, heartbeat emission, NACK retransmits. Default
5 ms (`DEFAULT_TICK_PERIOD`, spec-compliant). Lower = lower latency,
higher CPU. For hard-RT deployments use 1–2 ms; for soft-RT/non-RT,
50–100 ms trades latency for CPU headroom.

### `spdp_period`

How often we re-announce our participant on SPDP-multicast. Default
5 s — matches the DDS reference defaults. Discovery latency for a
new peer is at most one period.

### `spdp_multicast_group`

Default `239.255.0.1` (per RTPS spec). Override only if your
network filters this group; consider unicast static peer-list
when running in cloud / multi-VLAN where multicast is disabled.

### `multicast_interface`

Which local interface to bind for multicast joins. Default
`0.0.0.0` lets the kernel pick (works on most single-homed hosts).
Multi-homed hosts should pin to a specific interface, otherwise
discovery picks an arbitrary route.

### `participant_lease_duration`

Announced in SPDP as `PARTICIPANT_LEASE_DURATION`. After this
period without a heartbeat, peers consider this participant
dead. Default 100 s.

### `wlp_period`

Writer-Liveliness-Protocol tick. `Duration::ZERO` (the default)
means "lease/3" — three misses before the participant is declared
not-alive. Override for aggressive testing.

### `announce_secure_endpoints`

When `true`, the SPDP beacon announces 12 secure-discovery bits
(DDS-Security 1.2 §7.4.7.1). The DCPS factory flips this on
automatically when a `PolicyEngine` is configured. Available
without the `security` feature so tests can verify bit presence.

### `observability`

A `SharedSink = Arc<dyn Sink>` that receives lifecycle events (`user_writer.created`, `user_reader.created`,
`writer.matched_remote_reader`). Defaults to the no-op
`null_sink()`. Inject `StderrJsonSink::new()` for JSON-line stderr
output that Vector/fluentd/Datadog/journald consume directly.

### Security fields (`feature = "security"`)

| Field | Purpose |
|---|---|
| `security` | Optional `Arc<SharedSecurityGate>` — wraps every outbound + unwraps every inbound |
| `security_logger` | Optional `Arc<dyn LoggingPlugin>` for security audit events |
| `interface_bindings` | Multi-interface routing pool — per spec or per IP-range |

See [security.md](security.md) for the full security configuration.

## Common combinations

### Sandbox / test

```rust
RuntimeConfig::default()
```

5 ms tick, 5 s SPDP, no security, no observability.

### Production server with monitoring

```rust
RuntimeConfig {
    observability: Arc::new(StderrJsonSink::new()),
    ..Default::default()
}
```

### Hard real-time

```rust
RuntimeConfig {
    tick_period: Duration::from_millis(2),
    spdp_period: Duration::from_secs(60),  // discovery is rare
    participant_lease_duration: Duration::from_secs(10),
    ..Default::default()
}
```

Plus a `zerodds-rt-linux::SchedulerProfile::RealtimeFifo { priority: 60 }`
applied to your hot-path threads.
