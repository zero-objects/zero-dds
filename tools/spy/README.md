# zerodds-spy

Subscribe to a **DDS topic** and dump samples — like `tcpdump` for DDS.

The tool starts a `DcpsRuntime`, registers an untyped reader on the
named topic, and prints metadata + an optional hex snippet of every
incoming sample until you Ctrl-C, hit a sample-count limit, or a
duration elapses.

## Usage

```bash
# Subscribe to topic Foo on default domain (0) until Ctrl-C
zerodds-spy -t Foo

# Capture exactly 50 samples
zerodds-spy -t Foo -n 50

# 10s burst, dump only first 8 hex bytes per sample
zerodds-spy -t Foo --duration 10s --hex 8

# Subscribe on domain 5
zerodds-spy -d 5 -t Sensor/Stream/A
```

| Flag                | Meaning                                            | Default                |
|---------------------|----------------------------------------------------|------------------------|
| `-d, --domain`      | DDS Domain ID                                      | 0                      |
| `-t, --topic`       | Topic name (REQUIRED)                              | —                      |
| `-n, --count`       | Stop after N samples                               | unlimited              |
| `--duration`        | Stop after duration (`5`, `30s`, `2m`, `1h`)       | until SIGINT           |
| `-x, --hex`         | First BYTES of payload as hex (0 = no hex)         | 32                     |

## Output

Each sample is printed as one line:

```text
[     1] writer=000003c2 bytes=64 deadbeef cafebabe 12345678 90abcdef
[     2] writer=000003c2 bytes=64 deadbeef cafebabe 12345678 90abcdef
...
```

Lifecycle markers (Disposed / Unregistered) are printed as
`[lifecycle] <kind>`.

## Exit Codes

| Code | Meaning           |
|------|-------------------|
| 0    | Success           |
| 2    | CLI parse error   |
| 3    | DDS / I/O error   |
