# ZeroDDS-UDS-Transport 1.0 — Spec-Coverage

ZeroDDS-vendor-spezifischer Unix-Domain-Socket-Transport für
Container-IPC. **Nicht OMG-normativ** — Cyclone DDS und FastDDS haben
keinen offiziellen UDS-Transport.

| Spec-Family | Status |
|---|---|
| **OMG-normativ** | DDSI-RTPS 2.5 §9.4 LocatorKind (vendor-reserviert) — Locator-Wert in `crates/rtps/src/wire_types.rs` |
| **ZeroDDS-eigene Spec** | Path-Resolution + SOCK_DGRAM-Format + Abstract-Namespace — diese Datei |

## §1 Scope und Spec-Status

### §1.1 Was OMG normiert

DDSI-RTPS 2.5 §9.4 erlaubt vendor-reservierte Locator-Kinds. ZeroDDS
allokiert `LOCATOR_KIND_UDS = 0x81000001` (vendor-spezifischer Wert).

Kein normatives Wire-Format, kein Path-Schema, kein Cleanup-Protokoll.

### §1.2 ZeroDDS-Wahl

Eigene Spec für:
- Filesystem-Pfad-Resolution.
- SOCK_DGRAM-Wire-Format.
- Linux-Abstract-Namespace-Variante.
- Cleanup-Semantik.

## §2 Path-Resolution

### §2.1 Filesystem-Modus (Default)

16-Byte `Locator`-Adresse → Path:

```text
<base_dir>/<lowercase-hex32>.sock
```

- Default `base_dir` = `/tmp/zerodds/uds`.
- `<lowercase-hex32>` ist die 16-Byte-ID als 32-Zeichen-Hex.
- Base-Directory wird lazy auf Bind erstellt mit Mode `0700`
  (user-private).

Repo-Anker: `lib.rs::socket_path`, `lib.rs::DEFAULT_BASE_DIR`.

### §2.2 Abstract-Namespace-Modus (Linux-only)

Alternative auf Linux: Sockets im Abstract-Namespace (kein
Filesystem-Inode). Path:

```text
\0zd-<lowercase-hex32>
```

(Leading-Null-Byte signalisiert Abstract-Namespace bei Linux).

- Vorteile: kein File-Cleanup nötig, keine Inode-Permissions.
- Trade-off: nur Linux, kein Cross-Mount-Volume.

Repo-Anker: `abstract_dgram.rs::AbstractDgramSocket`.

## §3 Wire-Format

`SOCK_DGRAM` über UDS — eine Datagram-Message pro RTPS-Frame. Kernel
erhält Message-Grenzen (anders als TCP-Stream).

- Default `max_datagram` = 65 536 Bytes.
- Kernel-Limit: Linux `wmem_max` (default 212 992) ist die obere
  Grenze; ZeroDDS liegt drunter.

Repo-Anker: `lib.rs::DEFAULT_MAX_DATAGRAM`,
`lib.rs::UdsTransport::recv`.

## §4 Cleanup-Semantik

### §4.1 Filesystem-Modus

- Bind erstellt Socket-File auf-demand.
- Drop entfernt das Socket-File via `fs::remove_file` (best-effort).
- Crash: Zombie-Socket bleibt; nächster Bind detektiert das via
  `path.exists()` und failed mit `AlreadyInUse` (TOCTOU-sicher: erst
  Probe, dann fail-fast — kein Auto-Cleanup, weil Auto-Cleanup
  beziehbar wäre als Cross-Process-Race).

### §4.2 Abstract-Modus

Kein Cleanup nötig — Linux räumt Abstract-Sockets beim Close des
letzten FD automatisch.

## §5 Container-Use-Case

### §5.1 Docker/Kubernetes-Pattern

Gemountetes Volume `/tmp/zerodds/uds` zwischen zwei Containern; jeder
Container bindet seinen eigenen 16-Byte-Locator als Socket-File. Der
Kernel routet zwischen den FDs.

### §5.2 Wann UDS statt SHM

- Multicast geblockt (Cluster-Network-Policy).
- POSIX-SHM cross-Container unpraktisch (UID-Mapping, `/dev/shm`-
  Sichtbarkeit, SELinux-Profile).
- Volume-Mount ist die realistische Permission-Boundary.

## §6 Cross-Vendor-Interop

**Nicht vorgesehen**. UDS ist intra-Container/intra-Host-IPC. Cross-
Vendor-Interop mit Cyclone/FastDDS bleibt UDP/TCP/SHM-Domain.

## §7 Plattform-Support

| Plattform | Status | Anmerkungen |
|---|---|---|
| Linux | ✅ primary | Filesystem + Abstract Namespace |
| macOS | ✅ supported | Nur Filesystem-Modus (kein Abstract Namespace) |
| Windows | ❌ nicht supported | UDS ist Unix-spezifisch (Windows-Named-Pipes wären die Analogie) |

## §8 Test-Coverage

| Spec-Sektion | Tests |
|---|---|
| §2.1 Filesystem-Path | `lib.rs::tests::socket_path_*` |
| §2.2 Abstract-Namespace | `abstract_dgram.rs::tests::abstract_*` |
| §3 SOCK_DGRAM | `lib.rs::tests::send_recv_*` |
| §4 Cleanup | `lib.rs::tests::cleanup_*`, `bind_existing_path_*` |
| §5 Cross-Process | `tests/l1_cross_process.rs` |

Total: 16 lib + 1 cross-process = 17 Tests grün.

## §9 Status

**Voll abgedeckt**. ZeroDDS-UDS-Transport ist eine vollständige,
in-sich-kohärente Spec; alle §-Sektionen sind implementiert und
getestet.
