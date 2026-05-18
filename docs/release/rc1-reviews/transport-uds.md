# RC1 Review — `zerodds-transport-uds`

> **Referenz:** `docs/release/RC1_GUARDRAILS.md`.
> **Layer:** 2.8 (Wire — Unix Domain Sockets)
> **Reviewer:** Claude
> **Public-Strategy:** 🌐 public

---

## 1 Purpose

ZeroDDS-Unix-Domain-Socket-Transport — Container-IPC. Default
SOCK_DGRAM über Filesystem-Sockets, optional Linux Abstract-Namespace.

## 2 Public-Strategy

🌐 public — Container-IPC ohne Multicast-Voraussetzung.

## 3 Content-Inventur

### 3.1 Module

```
src/lib.rs            # UdsTransport (Filesystem-Modus, ~498 LOC)
src/abstract_dgram.rs # AbstractDgramSocket (Linux-only, ~601 LOC)
```

### 3.2 Public-API-Surface

```rust
pub struct UdsTransport;
pub struct UdsConfig;
pub fn socket_path(base_dir, id) -> PathBuf;
pub const DEFAULT_BASE_DIR: &str;
pub const DEFAULT_MAX_DATAGRAM: usize;
pub struct UdsAbstractDgramTransport;
pub struct AbstractDgramConfig;
pub enum UdsAddress;
pub const DEFAULT_RECV_BUF: usize;
pub const MAX_ABSTRACT_NAME: usize;
```

### 3.3 Tests

- `cargo test -p zerodds-transport-uds`: ✅ 17 passed (16 lib + 1 cross-process integration).

### 3.4 Coherence-Audit (§1.5b)

| Public-Item | Spec-Anker | External Production-Refs | Klassifikation | Decision |
|---|---|---|---|---|
| `UdsTransport` | ZeroDDS-UDS-Transport 1.0 §3 | 2 (tools/isolation-smoke, tests/l1) | CONNECTED | — |
| `UdsConfig` | ZeroDDS-UDS-Transport 1.0 §2 | 2 | CONNECTED | — |
| `UdsAbstractDgramTransport` | ZeroDDS-UDS-Transport 1.0 §2.2 (Linux Abstract Namespace) | 2 | CONNECTED | — |
| `AbstractDgramConfig` | analog | 2 | CONNECTED | — |
| `UdsAddress` | ZeroDDS-UDS-Transport 1.0 §2 | 2 | CONNECTED | — |
| `socket_path` (fn) | ZeroDDS-UDS-Transport 1.0 §2.1 (Path-Resolution) | 0 ext, 4 doc | VENDOR-EXTENSION (Public-API-Helper für End-User-Path-Inspection) | — |
| `DEFAULT_BASE_DIR` | Library-Default `/tmp/zerodds/uds` | 0 ext, 2 doc | VENDOR-EXTENSION (Public Default-Konstante) | — |
| `DEFAULT_MAX_DATAGRAM` | Library-Default 65 536 | 0 ext, 2 doc | VENDOR-EXTENSION | — |
| `DEFAULT_RECV_BUF` | Library-Default für Abstract-Namespace | 0 ext | VENDOR-EXTENSION | — |
| `MAX_ABSTRACT_NAME` | Linux-Kernel-Limit-Konstante | 0 ext | VENDOR-EXTENSION | — |

**Zusammenfassung:** 10/10 Public-Items klassifiziert. 0 ❌-Klassen.

## 4 Wiring

### 4.1 Dependencies

```toml
zerodds-rtps = { path = "../rtps" }
zerodds-transport = { path = "../transport" }
socket2 = { version = "0.5", features = ["all"] }
libc = "0.2"
```

### 4.2 Dependents

`tools/isolation-smoke`, `tests/l1_cross_process.rs`.

### 4.3 Feature-Flags

| Feature | Default | Zweck |
|---|---|---|
| `std` | ✅ | UnixDatagram |
| `alloc` | ✅ (via std) | — |

## 5 Spec-Relevanz

- **Spec:** ZeroDDS-UDS-Transport 1.0 (`docs/spec-coverage/zerodds-uds-transport-1.0.md`).

## 6 Cleanup-Findings

Keine — clean nach Layer-2-Pass.

## 7 Cleanup-Actions

1. SPDX-Header in 2 src-Files.
2. Cargo.toml RC1-Metadata.
3. Internal-Review-Cycle-Marker (`phase2-0-*`) gestrippt.
4. Crate-Header rewrite mit Spec-Anker.
5. ZeroDDS-UDS-Transport-1.0-Spec materialisiert.

## 8 Spec-Doc-Updates

Neuer Spec-Doc `docs/spec-coverage/zerodds-uds-transport-1.0.md` (§1-§9).

## 9 Doc-Artefacts

- [x] Cargo.toml RC1
- [x] lib.rs-Header mit Spec
- [x] README + CHANGELOG

## 10 Tests + Lints + Doc-Build

```bash
cargo test -p zerodds-transport-uds                                  # ✅ 17 passed
cargo clippy -p zerodds-transport-uds --all-targets -- -D warnings   # ✅
cargo doc -p zerodds-transport-uds --no-deps                         # ✅
```

## 11 RC1-DoD-Checkliste

- [x] §1.1-§1.13 alle ✅

## 12 Sign-off

- **Crate-Version:** `1.0.0-rc.1`
- **Reviewer:** Claude
