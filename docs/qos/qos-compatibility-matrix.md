# QoS Compatibility Matrix

**Erzeugt von `qos-matrix`** — Durability × Reliability × Ownership.

* Kompatible Kombinationen: `18` / `64`
* `✓` = `check_compatibility().is_compatible() == true`.
* `✗` = inkompatibel; Details im Anhang.

| Writer \ Reader | TransientLocal/BestEffort/Exclusive | TransientLocal/BestEffort/Shared | TransientLocal/Reliable/Exclusive | TransientLocal/Reliable/Shared | Volatile/BestEffort/Exclusive | Volatile/BestEffort/Shared | Volatile/Reliable/Exclusive | Volatile/Reliable/Shared |
|---|---|---|---|---|---|---|---|---|
| Volatile/BestEffort/Shared | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ |
| Volatile/BestEffort/Exclusive | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ | ✗ |
| Volatile/Reliable/Shared | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ |
| Volatile/Reliable/Exclusive | ✗ | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ | ✗ |
| TransientLocal/BestEffort/Shared | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ |
| TransientLocal/BestEffort/Exclusive | ✓ | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ | ✗ |
| TransientLocal/Reliable/Shared | ✗ | ✓ | ✗ | ✓ | ✗ | ✓ | ✗ | ✓ |
| TransientLocal/Reliable/Exclusive | ✓ | ✗ | ✓ | ✗ | ✓ | ✗ | ✓ | ✗ |

## Inkompatibilitaeten (Details)

* `Volatile/BestEffort/Shared → Volatile/BestEffort/Exclusive`: Ownership
* `Volatile/BestEffort/Shared → Volatile/Reliable/Shared`: Reliability
* `Volatile/BestEffort/Shared → Volatile/Reliable/Exclusive`: Reliability, Ownership
* `Volatile/BestEffort/Shared → TransientLocal/BestEffort/Shared`: Durability
* `Volatile/BestEffort/Shared → TransientLocal/BestEffort/Exclusive`: Durability, Ownership
* `Volatile/BestEffort/Shared → TransientLocal/Reliable/Shared`: Durability, Reliability
* `Volatile/BestEffort/Shared → TransientLocal/Reliable/Exclusive`: Durability, Reliability, Ownership
* `Volatile/BestEffort/Exclusive → Volatile/BestEffort/Shared`: Ownership
* `Volatile/BestEffort/Exclusive → Volatile/Reliable/Shared`: Reliability, Ownership
* `Volatile/BestEffort/Exclusive → Volatile/Reliable/Exclusive`: Reliability
* `Volatile/BestEffort/Exclusive → TransientLocal/BestEffort/Shared`: Durability, Ownership
* `Volatile/BestEffort/Exclusive → TransientLocal/BestEffort/Exclusive`: Durability
* `Volatile/BestEffort/Exclusive → TransientLocal/Reliable/Shared`: Durability, Reliability, Ownership
* `Volatile/BestEffort/Exclusive → TransientLocal/Reliable/Exclusive`: Durability, Reliability
* `Volatile/Reliable/Shared → Volatile/BestEffort/Exclusive`: Ownership
* `Volatile/Reliable/Shared → Volatile/Reliable/Exclusive`: Ownership
* `Volatile/Reliable/Shared → TransientLocal/BestEffort/Shared`: Durability
* `Volatile/Reliable/Shared → TransientLocal/BestEffort/Exclusive`: Durability, Ownership
* `Volatile/Reliable/Shared → TransientLocal/Reliable/Shared`: Durability
* `Volatile/Reliable/Shared → TransientLocal/Reliable/Exclusive`: Durability, Ownership
* `Volatile/Reliable/Exclusive → Volatile/BestEffort/Shared`: Ownership
* `Volatile/Reliable/Exclusive → Volatile/Reliable/Shared`: Ownership
* `Volatile/Reliable/Exclusive → TransientLocal/BestEffort/Shared`: Durability, Ownership
* `Volatile/Reliable/Exclusive → TransientLocal/BestEffort/Exclusive`: Durability
* `Volatile/Reliable/Exclusive → TransientLocal/Reliable/Shared`: Durability, Ownership
* `Volatile/Reliable/Exclusive → TransientLocal/Reliable/Exclusive`: Durability
* `TransientLocal/BestEffort/Shared → Volatile/BestEffort/Exclusive`: Ownership
* `TransientLocal/BestEffort/Shared → Volatile/Reliable/Shared`: Reliability
* `TransientLocal/BestEffort/Shared → Volatile/Reliable/Exclusive`: Reliability, Ownership
* `TransientLocal/BestEffort/Shared → TransientLocal/BestEffort/Exclusive`: Ownership
* `TransientLocal/BestEffort/Shared → TransientLocal/Reliable/Shared`: Reliability
* `TransientLocal/BestEffort/Shared → TransientLocal/Reliable/Exclusive`: Reliability, Ownership
* `TransientLocal/BestEffort/Exclusive → Volatile/BestEffort/Shared`: Ownership
* `TransientLocal/BestEffort/Exclusive → Volatile/Reliable/Shared`: Reliability, Ownership
* `TransientLocal/BestEffort/Exclusive → Volatile/Reliable/Exclusive`: Reliability
* `TransientLocal/BestEffort/Exclusive → TransientLocal/BestEffort/Shared`: Ownership
* `TransientLocal/BestEffort/Exclusive → TransientLocal/Reliable/Shared`: Reliability, Ownership
* `TransientLocal/BestEffort/Exclusive → TransientLocal/Reliable/Exclusive`: Reliability
* `TransientLocal/Reliable/Shared → Volatile/BestEffort/Exclusive`: Ownership
* `TransientLocal/Reliable/Shared → Volatile/Reliable/Exclusive`: Ownership
* `TransientLocal/Reliable/Shared → TransientLocal/BestEffort/Exclusive`: Ownership
* `TransientLocal/Reliable/Shared → TransientLocal/Reliable/Exclusive`: Ownership
* `TransientLocal/Reliable/Exclusive → Volatile/BestEffort/Shared`: Ownership
* `TransientLocal/Reliable/Exclusive → Volatile/Reliable/Shared`: Ownership
* `TransientLocal/Reliable/Exclusive → TransientLocal/BestEffort/Shared`: Ownership
* `TransientLocal/Reliable/Exclusive → TransientLocal/Reliable/Shared`: Ownership
