# Architecture Decision Records

Non-trivial architecture decisions are documented here as ADRs.
Binding according to `docs/architecture/07_risks_and_strategy.md §4.2 Bus-Factor`.

## Numbering

ADRs are numbered sequentially as `NNNN-kurzer-titel.md`, starting at `0001`.
A number once assigned is not reused — superseded ADRs
remain and point to their successor.

## Template

New ADRs start from `_template.md` as the template to copy.

## Status lifecycle

```
proposed  ->  accepted  ->  superseded
                     \->  rejected
                     \->  deprecated
```

## Index

| No. | Title | Status | Date |
|---|---|---|---|
| 0001 | [Vendor spec strategy for gaps in the OMG ecosystem](0001-vendor-spec-strategie.md) | accepted | 2026-05-04 |
| 0002 | [async DDS API runtime-agnostic, with Tokio glue as an option](0002-async-api-runtime-agnostic.md) | accepted | 2026-05-04 |
| 0003 | [Flatdata backend trait (in-memory + POSIX mmap)](0003-flatdata-backend-trait.md) | accepted | 2026-05-04 |
| 0004 | [Iceoryx2 as an optional backend (build + config)](0004-iceoryx2-bridge-optional.md) | accepted | 2026-05-04 |
| 0005 | [Flatdata dual stack: DCPS integration as an opt-in feature](0005-flatdata-dual-stack-opt-in.md) | accepted | 2026-05-04 |
| 0006 | [PID_SHM_LOCATOR as vendor PID 0x8001](0006-pid-shm-locator-vendor-pid.md) | accepted | 2026-05-04 |
