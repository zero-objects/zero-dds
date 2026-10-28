# combined-keyed-topic

**Construct** — a keyed topic type

The shape a real topic has, so the corpus is not only unit constructs: two keys of different kinds, a nested final aggregate, a narrow enum, a bounded sequence and an absent optional, all inside an appendable type in a module. Both the payload and the KeyHash are worth measuring here, and the KeyHash takes the two keys in member-id order regardless of where they sit in the payload.

## Probe status

| run | `wireprobe-zerodds` |
|---|---|
| `--encoding xcdr2 --endian le` | 69 bytes — `4100000008000000722dc3bc2d3031007929edff000000000000f83f00000000000004c00200000003000000000000000000c03f00000000000013c0000000205fa0024200` |
| `--encoding xcdr2 --endian be` | 69 bytes — `0000004100000008722dc3bc2d303100ffed29793ff8000000000000c00400000000000002000000000000033fc0000000000000c0130000000000004202a05f2000000000` |
| `--encoding xcdr1 --endian le` | 68 bytes — `08000000722dc3bc2d3031007929edff000000000000f83f00000000000004c00200000003000000000000000000c03f00000000000013c0000000205fa0024205000000` |
| `--encoding xcdr1 --endian be` | 68 bytes — `00000008722dc3bc2d303100ffed29793ff8000000000000c00400000000000002000000000000033fc0000000000000c0130000000000004202a05f2000000000050000` |
| `--emit keyhash` | 16 bytes — `23f288d176d8caf8aba3caa178f7f69f` |

The bytes are what the Rust probe produces today, recorded so that this
case is known to have been run and not merely written. They are not a
reference: the reference is the foreign implementation's own output, which
the suite compares against.
