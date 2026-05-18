# Soak-Host `pivot` — Setup-Anleitung

Stand 2026-05-01. Konfiguration für CI-4 (`soak-pivot`-Job in
`.gitlab-ci.yml`).

## Inventar

| Eigenschaft | Wert |
|---|---|
| Hostname | `pivot` (intern erreichbar als `pivot`) |
| OS | Debian Trixie via Proxmox-LXC (Kernel 6.17.13-1-pve) |
| CPU | 20 Cores |
| RAM | 128 GB |
| Disk | 451 GB total, 417 GB free per 2026-05-01 |

## Vorinstalliert (geprüft 2026-05-01)

* `rustup` mit `rustc 1.95.0` (root-installiert unter `/root/.cargo/`)
* `python3` (3.11+)
* `git`
* `ps` (procps)

## Erst-Bootstrap (one-time)

### 1. Bench-User anlegen (NICHT root für CI)

```bash
ssh root@pivot

# Account fuer Soak-Runs
adduser --disabled-password --gecos 'ZeroDDS Soak' bench
mkdir -p /home/bench/.ssh
chmod 700 /home/bench/.ssh

# rustup user-local
sudo -u bench bash -c '
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
      --default-toolchain 1.85.0 --profile minimal
  echo "export PATH=\$HOME/.cargo/bin:\$PATH" >> ~/.bashrc
'

# (Optional: heaptrack für tiefere Heap-Profiling-Phase CI-4b)
apt update
apt install -y heaptrack
```

### 2. SSH-Deploy-Key

```bash
# Auf der Workstation:
ssh-keygen -t ed25519 -f ~/.ssh/zerodds_soak_deploy -C "gitlab-ci-zerodds-soak"

# Public-Key auf den pivot-Host pushen
ssh root@pivot "cat >> /home/bench/.ssh/authorized_keys" \
    < ~/.ssh/zerodds_soak_deploy.pub
ssh root@pivot "chown -R bench:bench /home/bench/.ssh && \
                chmod 600 /home/bench/.ssh/authorized_keys"

# Host-Key fingerprint extrahieren
ssh-keyscan -t ed25519 pivot
```

### 3. GitLab-CI/CD-Variablen

In Project → Settings → CI/CD → Variables (Maintainer-only, protected):

| Variable | Type | Wert |
|---|---|---|
| `PIVOT_SOAK_SSH_KEY` | File | Inhalt von `~/.ssh/zerodds_soak_deploy` (Private Key) |
| `PIVOT_SOAK_HOST_KEY` | Variable | Output von `ssh-keyscan -t ed25519 pivot` |
| `PIVOT_SOAK_HOST` | Variable | `pivot` (oder voller FQDN) |
| `PIVOT_SOAK_USER` | Variable | `bench` |

### 4. Pipeline-Schedule (nightly 02:00 UTC)

Project → CI/CD → Schedules → Neuer Schedule:

* **Description:** `Nightly soak-pivot 24h`
* **Cron:** `0 2 * * *`
* **Branch:** `main`
* **Variables:** `RUN_SOAK=true`

## Triggers

* **Schedule:** nightly 02:00 UTC (Default)
* **Pipeline-Variable:** `RUN_SOAK=true` beim Pipeline-Start
* **Manuell:** Klick auf den `soak-pivot`-Job in der UI (nur auf main)

## Lokales Smoke-Test (5 min, nicht 24 h)

Auf dem bench-User:

```bash
RUNTIME_SECS=300 SAMPLE_INTERVAL_SECS=10 \
WORKDIR=/tmp/zerodds-soak-smoke \
bash tests/perf/soak_runner.sh
```

Output unter `/tmp/zerodds-soak-smoke/soak-output/`:
* `rss-timeline.csv` — RSS + sample-count pro 10s
* `soak-summary.json` — strukturiert
* `soak-summary.md` — Markdown mit Verdict
* `pub.log`, `sub.log` — Process-Stdout

## Was der Job liefert

Artefakt `soak-out/` (90 Tage Retention):

* `soak-summary.md` — Verdict + RSS-Wachstum pro Prozess
* `soak-summary.json` — strukturierte Daten zum Parsen / Trend-Plot
* `rss-timeline.csv` — RSS+sample-count alle 60s
* `pub.log`, `sub.log` — Stdout der Prozesse
* `build.log` — cargo-build-Output

## Verdict-Kriterien

PASS, wenn:
1. RSS-Wachstum (median early-steady vs median late-steady) ≤ 25%
   für sowohl Publisher als auch Subscriber
2. Mindestens ein Sample empfangen
3. Kein Sample-Stillstand > 5 × `SAMPLE_INTERVAL_SECS`
4. Beide Prozesse leben durch den ganzen Run

FAIL sonst — exit-code != 0, Job rot, Artefakte trotzdem hochgeladen
(`when: always`).

## CI-4b heaptrack-Mode (2026-05-01)

`soak-pivot-heaptrack`-Job ist live: identisch zu `soak-pivot`, aber
mit `HEAPTRACK=1` Environment-Variable. Setup auf pivot-Host:

```bash
ssh root@pivot
apt install -y heaptrack heaptrack-gui   # gui optional
```

Output zusätzlich zu den normalen Soak-Files:
* `heaptrack-pub.{zst,gz}` — Heaptrack-Capture-File des Publishers
* `heaptrack-sub.{zst,gz}` — Heaptrack-Capture-File des Subscribers
* `heaptrack-{pub,sub}.txt` — vollständige `heaptrack_print`-Analyse
* `heaptrack-{pub,sub}.summary.txt` — erste 200 Zeilen (Top-Allocators)

**Default-Runtime: 4 h** statt 24 h (heaptrack hat ~20-30 % Overhead).
Override via `SOAK_RUNTIME_SECS`-Pipeline-Var.

**Trigger:** `RUN_SOAK_HEAPTRACK=true`-Pipeline-Var oder manueller
Job-Klick. Vor Performance/Memory-Releases gezielt fahren.

## Folge-Welle CI-4c

* **valgrind --tool=massif**: alternativer Heap-Profiler, robuster für
  long-running.
* **Multi-Endpoint-Soak**: aktuell nur ein Pub + ein Sub. Zukunft:
  100 Endpoints parallel, Discovery-Stabilität-Check.
* **Cross-Vendor-Soak**: ZeroDDS-Sub gegen Cyclone-Pub 24h via
  `ddsperf pub` als Pub-Seite.
