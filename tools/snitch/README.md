# zerodds-snitch

DDS **discovery probe**. Listens on a domain for SPDP/SEDP traffic for a
configurable window then prints all discovered participants and the
endpoint counts seen.

## Usage

```bash
# 5s probe of domain 0
zerodds-snitch

# 10s probe of domain 5, JSON output
zerodds-snitch --duration 10s -d 5 -f json
```

| Flag                | Meaning                                       | Default          |
|---------------------|-----------------------------------------------|------------------|
| `-d, --domain`      | DDS Domain ID                                 | 0                |
| `--duration`        | Probe duration (`5`, `30s`, `2m`, `1h`)       | 5s               |
| `-f, --format`      | `text` or `json`                              | text             |

## Output

Text format:

```text
=== Discovery snapshot ===
participants:       2
publications seen:  4
subscriptions seen: 5

participants:
  · prefix=01020304050607080900aabb vendor=0117 key=0102030405060708...
  · prefix=01020304050607080900aabb vendor=01ff key=...
```

JSON format (single line, machine readable):

```json
{"participants":2,"publications":4,"subscriptions":5,"entries":[{"prefix":"...","vendor":"0117","key":"...","user_data_bytes":0}]}
```

## Exit Codes

| Code | Meaning           |
|------|-------------------|
| 0    | Success           |
| 2    | CLI parse error   |
| 3    | DDS / I/O error   |
