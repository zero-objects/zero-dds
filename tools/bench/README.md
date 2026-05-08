# zerodds-bench

End-to-End latency and throughput benchmarking on the **ZeroDDS DCPS pipeline**.

Both modes use a single-process loopback: one `UserWriter` and one
`UserReader` on the same topic in the same `DcpsRuntime`. This exercises
the full stack — `HistoryCache`, XCDR2 encapsulation, RTPS-2.5 wire
protocol, UDP transport — without external setup.

## Sub-Commands

```text
zerodds-bench latency    [-d DOMAIN] [-t TOPIC] [-p PAYLOAD] [--duration DUR]
zerodds-bench throughput [-d DOMAIN] [-t TOPIC] [-p PAYLOAD] [--duration DUR]
zerodds-bench info
```

| Flag                | Meaning                                            | Default                     |
|---------------------|----------------------------------------------------|-----------------------------|
| `-d, --domain`      | DDS Domain ID                                      | 0                           |
| `-t, --topic`       | Topic name                                         | `zerodds/bench/loopback`    |
| `-p, --payload`     | Payload size (bytes)                               | 64                          |
| `--duration`        | Run duration (`5`, `30s`, `2m`, `1h`)              | 5s                          |
| `-h, --help`        | Show help                                          |                             |
| `-V, --version`     | Print version                                      |                             |

## Examples

```bash
# Quick 5s latency check (default 64-byte payload)
zerodds-bench latency

# 30s throughput at 1 KiB payload
zerodds-bench throughput -p 1024 --duration 30s

# Capabilities report
zerodds-bench info
```

## Output

`latency` prints sample count plus min / p50 / p99 / max in microseconds:

```text
samples=4521 min=12.3us p50=18.7us p99=42.1us max=83.6us
```

`throughput` prints sent + received counts plus rate in messages/s and MiB/s:

```text
sent=152340 received=152340 elapsed=5.00s rate=30468msgs/s 1.86MiB/s
```

## Exit Codes

| Code | Meaning           |
|------|-------------------|
| 0    | Success           |
| 2    | CLI parse error   |
| 3    | DDS / I/O error   |

## Spec & Source

Pipeline: `crates/dcps/src/runtime.rs` → `crates/rtps/` → `crates/transport-udp/`.

For deeper apples-to-apples comparison vs. Cyclone DDS / RTI / FastDDS, see
the internal harness in `tools/bench-suite/`.
