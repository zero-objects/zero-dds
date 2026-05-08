# zerodds-mq

Cross-domain DDS bridge — pumps **raw bytes between two DDS domains**
without re-encoding. Useful for connecting environments that are
isolated by domain ID (e.g. `dev` on domain 0, `prod` on domain 5).

The bridge starts two `DcpsRuntime` instances (one per domain), reads
samples from the source topic, and republishes them on the destination
topic.

## Usage

```bash
# One-way bridge: domain 0 → domain 5, same topic name
zerodds-mq -t Sensor/Stream/A --src-domain 0 --dst-domain 5

# Different topic name on each side
zerodds-mq --src-domain 0 --dst-domain 5 \
           --src-topic LegacyName --dst-topic NewName

# Bidirectional (both domains see all traffic)
zerodds-mq -t Foo --src-domain 0 --dst-domain 1 --bidirectional

# Stop after 60s
zerodds-mq -t Foo --src-domain 0 --dst-domain 1 --duration 60s
```

| Flag                  | Meaning                                                  | Default          |
|-----------------------|----------------------------------------------------------|------------------|
| `--src-domain`        | Source domain ID                                         | 0                |
| `--dst-domain`        | Destination domain ID (must differ from src)             | 1                |
| `-t, --topic`         | Same topic name on both sides                            | —                |
| `--src-topic`         | Source topic (overrides `-t`)                            | —                |
| `--dst-topic`         | Destination topic (overrides `-t`)                       | —                |
| `--duration`          | Stop after duration                                      | until SIGINT     |
| `--bidirectional`     | Mirror traffic in both directions                        | off              |

## Behaviour

The bridge is **untyped**: it preserves the encoded payload byte-for-
byte. Any DDS data type works as long as both domains agree on the
encoding (XCDR2 LE @final by default).

QoS used for the bridge endpoints:
- Reliability: **RELIABLE**
- Durability: **VOLATILE**
- History: writer KeepLast(32)

## Exit Codes

| Code | Meaning           |
|------|-------------------|
| 0    | Success           |
| 2    | CLI parse error   |
| 3    | DDS / I/O error   |
