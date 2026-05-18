# ZeroDDS Roadmap RC1 → 1.0.0-final

Master-Index der Roadmap-Phasen. Jeder Track hat eine eigene Detail-Doku
unter diesem Verzeichnis. Phasen sind sequentiell, Tracks innerhalb einer
Phase parallelisierbar.

```
roadmap/
├── README.md                                  ← dieses Dokument
│
├── PHASE_RC1_stabilize.md                     (jetzt aktiv — finalize rc.1 release)
│
├── PHASE_RC2_pre10_major.md                   (große Pre-1.0 Tracks)
│   ├── track-datalake.md                      ← konfigurierbare Tiered-Storage-Engine
│   ├── track-amqp-09-rabbitmq.md              ← AMQP 0.9.1 als zweites Bridge-Target
│   ├── track-audit-demos.md                   ← alle Demos durchgehen + testen
│   ├── track-audit-tutorials.md               ← alle Tutorials durchgehen + testen
│   └── track-audit-micro-profiles.md          ← no_std + Cortex-M + WASM-server
│
├── PHASE_RC3_breadth.md                       (Sprach-Bindings + DDS-Stack-Komplettierung)
│   ├── track-languages-go-swift-kotlin-zig.md
│   ├── track-dds-routing-service.md           ← OMG DDS-RS Spec
│   └── track-dds-persistence-service.md       ← OMG DDS-PS Spec
│
├── PHASE_10_final.md                          (1.0.0-final Gate)
│   ├── track-omg-vendor-id.md                 ← Vendor-ID-Vergabe abwarten
│   ├── track-news-section-launch.md           ← News-Sektion auf der Website füllen
│   └── track-marketing-launch.md              ← coordinated 1.0 announcement
│
└── POST_10_backlog.md                         (langfristige Strategie)
    ├── track-cloud-native.md                  ← K8s Operator, Helm, Service-Mesh
    ├── track-security-compliance.md           ← CRA, SLSA-3, FIPS-140, DO-178C
    ├── track-industry-verticals.md            ← AUTOSAR, Aerospace, Automotive
    └── track-realtime-edge.md                 ← PREEMPT_RT, AUTOSAR-CP, MCU
```

## Phase-Übersicht

| Phase | Goal | Exit-Kriterium | Estimate |
|---|---|---|---|
| **RC1-stabilize** | rc.1 Tag aus dem Workspace fahren, Stage-1-Distros live | Tag `1.0.0-rc.1`, alle 12 Distro-Channels grün | aktiv, ~Tage |
| **RC2** (`1.0.0-rc.2`) | Datalake + AMQP 0.9 + Audit-Tracks (Demos/Tutorials/Micro-Profile) | Datalake operational + 0.9-Bridge + alle Demos getestet | 4-8 Wochen |
| **RC3** (`1.0.0-rc.3`) | Sprach-Bindings Go/Swift/Kotlin/Zig, DDS-RS + DDS-PS | 4 neue Sprachen + 2 neue Services | 8-12 Wochen |
| **1.0-final** | OMG-Vendor-ID erteilt, News-Sektion launched | OMG bestätigt + öffentliche 1.0-Ankündigung | gated auf OMG |
| **Post-1.0** | Cloud-Native + Compliance + Vertical-Profile + Edge-RT | gestaffelt nach Markt-Demand | 2026-2027 |

## Tracking-Konvention

Pro Track:
- **Goal** — der eine Satz, was am Ende rauskommt
- **In-Scope** — was wir tun
- **Out-of-Scope** — was explizit nicht (mit Begründung)
- **Acceptance** — wie wir wissen dass es fertig ist
- **Estimate** — Personenwochen
- **Dependencies** — was muss vorher fertig sein
- **Status** — `📋 todo` / `🔄 in-progress` / `✅ done` / `🚫 blocked`

Statuswechsel werden als Doku-Edit + git-Commit getrackt — keine separate
Tracker-Tabelle, das Markdown-File ist die Single-Source-of-Truth.

## Was NICHT auf dem Plan steht

Bewusste Ausschlüsse mit Begründung — falls jemand fragt:

| Item | Warum nicht | Wann sinnvoll |
|---|---|---|
| Eigener Blog | "Albern für ein DDS-Projekt" — Käufer-Audience liest keine Blogs, sondern Specs + Conformance-Tests | nie |
| News-Sektion vor 1.0-final | Vor OMG-Vendor-ID-Vergabe haben wir nichts wirklich Neues anzukündigen | ab 1.0-final |
| Authenticode-Cert für Windows | Cost zu hoch für rc-Phase | nach 1.0 wenn Adoption es rechtfertigt |
| Long-Double-Software-Emulation in IDL | Rust hat kein native f80/f128, Software-Emulation unverhältnismäßig | n/a (rejected ADR) |
| MSIX (Microsoft Store) | irrelevant für ZeroDDS-Audience | nie |
