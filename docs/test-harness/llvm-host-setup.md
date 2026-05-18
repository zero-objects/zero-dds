# Bench-Host `llvm` — Setup-Anleitung

Stand 2026-05-01. Konfiguration für CI-3-Welle (`bench-llvm`-Job in
`.gitlab-ci.yml`).

## Inventar

| Eigenschaft | Wert |
|---|---|
| Hostname | `llvm` (intern erreichbar als `llvm`) |
| OS | Debian 12 (Kernel 6.1.164) |
| CPU | 24 Cores |
| Disk | 456 GB total, 229 GB free per 2026-05-01 |
| User | `llvm` (Bench-Account) |

## Vorinstalliert (geprüft 2026-05-01)

* `ddsperf` (Cyclone DDS Tools)
* `libfastrtps.so.2.9.1` + `libfastcdr` (eProsima Fast-DDS)
* `libddsc.so.0.10.2` (Cyclone DDS)
* `rustup` mit `rustc 1.85.0` unter `~/.cargo/bin/`
* `git`, `python3`

## Erst-Bootstrap (falls Host neu aufgesetzt wird)

```bash
ssh llvm@llvm

# rustup user-local (kein root noetig)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --default-toolchain 1.85.0 --profile minimal
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc

# DDS-Tools (root-Aktion, einmalig)
sudo apt update
sudo apt install -y \
    cyclonedds-tools \
    libfastrtps-dev libfastcdr-dev g++ \
    git python3 python3-pip
```

## SSH-Deploy-Key für GitLab-CI

Der `bench-llvm`-Job in der GitLab-Pipeline holt die Bench-Daten via SSH.
Setup einmalig:

```bash
# Auf der Workstation: Deploy-Key generieren
ssh-keygen -t ed25519 -f ~/.ssh/zerodds_bench_deploy -C "gitlab-ci-zerodds-bench"

# Public-Key auf den Bench-Host pushen
ssh-copy-id -i ~/.ssh/zerodds_bench_deploy.pub llvm@llvm

# Host-Key fingerprint extrahieren (fuer Host-Key-Pinning in CI)
ssh-keyscan -t ed25519 llvm
```

In GitLab-Project → Settings → CI/CD → Variables (alle nur sichtbar für
Maintainers + protected, masked wo möglich):

| Variable | Type | Wert |
|---|---|---|
| `LLVM_BENCH_SSH_KEY` | File | Inhalt von `~/.ssh/zerodds_bench_deploy` (Private Key) |
| `LLVM_BENCH_HOST_KEY` | Variable | Output von `ssh-keyscan -t ed25519 llvm` (komplette Zeile) |
| `LLVM_BENCH_HOST` | Variable | `llvm` (oder voller FQDN falls Runner anderen Namespace) |
| `LLVM_BENCH_USER` | Variable | `llvm` |

## Trigger des Jobs

Optionen:

1. **Manuell** auf jedem Branch — Klick auf den `bench-llvm`-Job in der
   Pipeline-UI.
2. **Pipeline-Variable** beim Pipeline-Start: `RUN_BENCH_LLVM=true`.
3. **Schedule** (zu konfigurieren) — nightly auf main.

## Was der Job liefert

Artefakt `llvm-bench-out/` (30 Tage):

* `bench-output.log` — vollständige Criterion-Ausgabe (alle 8 Suiten)
* `criterion/<bench>/llvm-<sha>/estimates.json` — Bench-Daten
* `cyclone_ping.log` + `cyclone_pong.log` — ddsperf-Latenz-Test
* `cyclone_pub.log` + `cyclone_sub.log` — ddsperf-Throughput-Test
* `latency_cyclone.json` — geparste RTT-Histogramm (min/p50/p90/p99/p99.9/max)
* `throughput_cyclone.json` — geparste Throughput (kS/s, kB/s)
* `bench-summary.md` — Markdown-Zusammenfassung

## Folge-Welle CI-3b

Aktuell wird `ddsperf` Cyclone-Self-Bench gefahren — als Sanity + stabile
Mess-Referenz. Echter Cross-Vendor-Vergleich braucht:

* ZeroDDS-eigener `ddsperf`-kompatibler Pinger (gleiche Topic-IDs)
* Apex.AI `performance_test` mit cyclonedds + fastdds + ZeroDDS-Plugin
* Auswertung pro Vendor in `latency_<vendor>.json` / `throughput_<vendor>.json`

Schätzung: ~1-2 PT für Apex.AI-Setup + ZeroDDS-Plugin.
