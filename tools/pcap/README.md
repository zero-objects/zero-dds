# zerodds-pcap

Offline pcap parser — extracts and decodes **RTPS submessages** from
existing pcap captures (e.g. captured with `tcpdump`, `dumpcap`, or
Wireshark). No DDS runtime, no live capture.

The tool finds the `RTPS` magic word inside each packet's payload (so
it works for any L2/L3/L4 combination — Ethernet, Linux SLL, NULL —
without explicit parsing) and then runs the full ZeroDDS RTPS decoder.

## Usage

```bash
# Capture some DDS traffic with tcpdump
sudo tcpdump -i any -w /tmp/dds.pcap 'udp portrange 7400-7500'

# Print every RTPS frame
zerodds-pcap parse /tmp/dds.pcap

# Aggregate counts per submessage kind
zerodds-pcap stats /tmp/dds.pcap
```

## Sub-Commands

| Sub-Command         | Action                                                    |
|---------------------|-----------------------------------------------------------|
| `parse <FILE>`      | Print every RTPS frame with submessage list               |
| `stats <FILE>`      | Print aggregate counts per submessage kind                |

## Output Example

```text
pcap: linktype=1 snaplen=65535 version=2.4
[    1] ts=1715000000.123456 guid_prefix=01020304... submessages=2
    · INFO_TIMESTAMP
    · DATA
[    2] ts=1715000000.234567 guid_prefix=05060708... submessages=1
    · HEARTBEAT
done · frames=42 rtps=12
```

## Exit Codes

| Code | Meaning                            |
|------|------------------------------------|
| 0    | Success                            |
| 2    | CLI parse error                    |
| 3    | File / pcap / RTPS decode error    |

## Limits

- Only `libpcap` format is supported (PCAPNG is a future extension).
- VLAN-tagged frames work as long as `RTPS` magic is in the captured
  payload; jumbo frames work up to the configured snaplen.
