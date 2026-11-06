# zerodds-flatdata-1.0 — offene Punkte

Aggregat aus `zerodds-flatdata-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## Nicht-Ziel — Cross-Host-Zero-Copy (RDMA, DPDK)

**Status:** `open` — die Spec verweist auf eine eigene Spec, die es in `docs/specs/` nicht gibt; Cross-Host-Zero-Copy über RDMA oder DPDK hat weder Spec noch Code.

## §4.1 Backend signalisiert den Same-Host-Reader

**Status:** `partial` — ereignisgesteuert für das In-Memory-Backend (Condvar) und das POSIX-Backend unter Linux und macOS. Auf den BSDs und Windows pollt der Reader weiter; siehe „§4.1 Cross-Process-Notify auf nicht-Linux-POSIX".

## §4.1 Cross-Process-Notify auf nicht-Linux-POSIX

**Status:** `partial` — macOS ist gebaut und getestet; die BSDs haben kein Build-Ziel in der CI, ihr Reader pollt weiter.
