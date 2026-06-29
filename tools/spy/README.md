# zerodds-spy

Subscribe to a **DDS topic** and dump samples — like `tcpdump` for DDS.

The tool starts a `DcpsRuntime` and **type-follows** the topic: it discovers
each writer's real IDL type via SEDP and attaches one opaque reader per type
(so it sees typed topics like `cuas::Track`, not just `zerodds::RawBytes`).
It prints metadata + an optional hex snippet of every incoming sample until you
Ctrl-C, hit a sample-count limit, or a duration elapses. Late-joining writers
(and additional types on the same topic) are picked up automatically.

Pass `--type <NAME>` to announce a type directly and skip discovery — useful for
a writer that is silent or joins after the spy.

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
| `--decode`          | Decode samples to typed JSON (needs `--type-file`) | off                    |
| `--type-file`       | Out-of-band IDL providing the types to decode      | —                      |

## Decoded output (`--decode`)

With `--decode --type-file <IDL>`, each sample is decoded against its writer's
IDL type and printed as compact JSON instead of hex. Types resolve lazily per
discovered writer type and are cached; a sample whose type isn't in the IDL (or
that doesn't decode) falls back to the hex line.

```bash
zerodds-spy -t TrackTopic --decode --type-file tracks.idl
# [     1] writer=000003c2 type=cuas::Track {"id":42,"name":"bandit","xs":[7,-3,1000]}
```

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
