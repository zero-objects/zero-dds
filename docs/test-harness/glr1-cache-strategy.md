# glr1 — Cache-Strategie und Disk-Wachstum

Wie der CI-Cache auf glr1 organisiert ist, was wachsen kann, und
welche Cleanup-Mechanismen greifen.

## Drei Cache-Layer

| Layer | Pfad | Mechanismus | Geteilt zwischen | Größe |
|-------|------|-------------|-----------------|-------|
| Cargo-Registry | `$CI_PROJECT_DIR/.cargo-home/registry/{index,cache}` | GitLab-Cache (zip), keyed auf `Cargo.lock` | Branches mit gleichem `Cargo.lock` | ~500 MB-2 GB |
| Cargo target/ | `/cache/zerodds-target/<branch>/` | Docker-Volume per Job-Type | Stages innerhalb einer Pipeline + Pipelines auf demselben Branch+Job-Type | ~5-10 GB |
| CI-Image | GitLab-Project-Container-Registry | image-pull | alle Jobs | ~3-5 GB |

## Wichtigste Erkenntnis — `/cache` ist kein Bind-Mount

GitLab Runner Docker-Executor (`volumes = ["/cache"]`) erstellt
**per-Job-Type Docker-Volumes** mit deterministischen Namen
(`runner-<token>-cache-<mount-hash>`), nicht einen einzelnen
Bind-Mount auf den Host. Konsequenzen:

- `df /cache` auf glr1 zeigt nichts — die Daten leben unter
  `/var/lib/docker/volumes/runner-...-cache-c33bcaa1.../`
- Pro Job-Name (clippy, build-x86_64, test, coverage, ...) ein
  eigener Volume → target/ wird **per-Job-Type wiederverwendet**,
  nicht "einmal geteilt"
- Konsequenz fürs Wachstum: bis zu 8-10 unabhängige target/-Trees
  (alle workspace-bauenden Jobs)

## Wachstum über Zeit

| Faktor | Beitrag |
|--------|---------|
| 1 target/ pro Job-Type | × ~8 (clippy, build×3, test, coverage, bench-compile, live-interop) |
| × Branches (per `${CI_COMMIT_REF_SLUG}` Subdir) | linear |
| × Cargo.lock-Versionen (alle target/ subdirs gemeinsam aber Re-Compile bei Lock-Drift) | meist 1 aktiv pro Branch |

**Worst-Case Wachstum:** 8 Volumes × 50 Branches × 5 GB = 2 TB.
**Realistic Steady-State** (mit Cleanup): 50-150 GB.

## Cleanup-Layer

### Layer 1 — Inside-Volume-Cleanup (`before_script`)

In `.gitlab-ci.yml` `default.before_script`:

```bash
find /cache/zerodds-target -maxdepth 1 -mindepth 1 -type d -mtime +14 -exec rm -rf {} \;
touch "$ZERODDS_TARGET_VOLUME"  # active branch bleibt warm
```

- mtime auf dem Branch-target-Verzeichnis wird durch jede Pipeline neu gesetzt
- Branches die 14 Tage keine Pipeline hatten werden in jedem Volume
  gelöscht
- Läuft pro Pipeline-Job, also pro Job-Type-Volume separat

### Layer 2 — Disk-Watchdog (systemd timer auf glr1)

`/usr/local/sbin/glr1-cache-watchdog` + `glr1-cache-watchdog.timer`
(stündlich):

- WARN bei `/var/lib/docker/volumes/` ≥ 80 GB → syslog `user.warning`
- CRIT bei ≥ 140 GB → syslog `user.crit`
- Tripwire only — kein automatisches Löschen

```bash
# Status anschauen
sudo journalctl -t glr1-cache-watchdog --since "1 day ago"
```

### Layer 3 — Manuelle Job-Type-Volume-Eviction (selten)

Wenn ein Job permanent verschwindet (z.B. `bench-main` nach Refactor)
oder Disk-CRIT geworfen wird:

```bash
# Liste Volumes
sudo docker volume ls | grep cache

# Volume eines spezifischen Job-Types löschen
sudo docker volume rm runner-<id>-cache-<hash>

# Oder pauschal alle ungenutzten — VORSICHT, killt aktive Caches:
sudo docker volume prune -f
```

## Tuning

Schwellen anpassen in `/usr/local/sbin/glr1-cache-watchdog`:

```bash
WARN_GB=80
CRIT_GB=140
```

Cleanup-Alter anpassen in `.gitlab-ci.yml` `default.before_script`:

```bash
find /cache/zerodds-target ... -mtime +14   # Tage
```

## Disk-Spec glr1

- 287 GB total, 192 GB free (initial)
- ZFS-backed via vm-112-disk-1 auf pve `tank`-Pool

Bei 80 GB Volumes-Verbrauch + 83 GB OS = 163 GB von 287 GB → 124 GB free.
Bei 140 GB Volumes (CRIT) → 64 GB free, eng genug für Eingriff.
