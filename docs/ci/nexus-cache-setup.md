# Nexus-Cache-Integration fuer ZeroDDS-CI

**Ziel:** Pipeline-Laufzeit um Faktor 2-3x reduzieren durch Proxy-Caching
und persistente Tool-Binaries.

## Aktuelle Langsamkeit (vor dieser Aenderung)

| Problem | Impact pro Job |
|---------|----------------|
| `rust:1.85-bookworm` von docker.io bei jedem Job | ~20-60 s |
| `rustup component add rustfmt/clippy` per fmt/clippy-Job | ~5 s × 2 Jobs |
| `cargo install cargo-deny` (baut aus Source in 1.88-Toolchain) | ~60-90 s |
| `cargo install cargo-llvm-cov` (baut aus Source) | ~120-180 s |
| `target/` Cache wird fuer alle Jobs-Lint+Build+Test+Coverage gleich verwendet (grosser Blob, fragmentierend) | ~30-60 s Dekompression |

Summe Startup-Overhead pro Pipeline ohne Cache: ~4-8 Minuten.

## Aenderungen

### 1. Docker-Registry-Mirror (daemon-seitig, nicht CI-Config!)

**Wichtig:** Der Nexus-Proxy wird **nicht** als Image-URL in der
`.gitlab-ci.yml` verwendet. Stattdessen ist der Docker-Daemon auf dem
Runner-Host (`pve` für `glr1`) mit `registry-mirror` konfiguriert:

```json
# /etc/docker/daemon.json auf dem Runner-Host
{
  "registry-mirrors": ["http://nexus.amstk.internal:5000"],
  "insecure-registries": ["nexus.amstk.internal:5000"]
}
```

Damit pullt der Daemon `rust:1.85-bookworm` transparent via Nexus-Mirror
— die CI-Config selbst bleibt bei `rust:1.85-bookworm` (docker.io-Pfad).

Analog zu `IfynaNeu/finanzplanung`-Pipeline, die dieselbe Runner-Host-
Config nutzt.

Die CI/CD-Variable `RUST_IMAGE` sollte **ungesetzt** bleiben oder auf
`rust:1.85-bookworm` stehen — **nicht** auf `nexus.amstk.internal:5000/...`,
sonst versucht der Daemon einen direkten HTTPS-Pull gegen den
HTTP-Nexus-Port.

### 2. Zwei-Schicht-Cache-Strategie

Statt eines monolithischen `.cargo-cache`-Blocks jetzt zwei Alias-Anchors:

- **`.cargo-deps-cache`** (nur deps): `.cargo-home/registry/{index,cache}` +
  `.cargo-home/bin/`. Key = hashed `Cargo.lock` (unabhaengig vom Branch).
  Verwendet von `fmt`, `clippy`, `zerodds-lint`, `deny`.
- **`.cargo-target-cache`**: `.cargo-deps-cache` + zusaetzlich `target/`
  mit Branch-spezifischem Key (`target-${CI_COMMIT_REF_SLUG}`). Verwendet
  von `build-x86_64`, `no-std`, `test`, `coverage`, `live-interop`.

Aliase via YAML-Anchors (`*cargo-cache`, `*cargo-target-cache`).

### 3. Persistente CLI-Tools

Die vorher teuren `cargo install cargo-deny` / `cargo-llvm-cov` werden
jetzt per Shell-Guard nur ausgefuehrt, wenn das Binary nicht bereits im
gecachten `.cargo-home/bin/` liegt:

```yaml
script:
  - |
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then
      cargo install --locked cargo-llvm-cov --version 0.6.21
    else
      echo "Using cached cargo-llvm-cov $(cargo-llvm-cov --version)"
    fi
```

`before_script` exportiert `.cargo-home/bin` auf PATH.

### 4. Cargo-Sparse-Index via Nexus (bereits vorhanden)

`$NEXUS_CARGO_INDEX` wird weiterhin in `before_script` als
`source.crates-io`-Replacement verwendet.

## Erwartete Einsparungen

| Phase | Vorher | Nachher (warmer Cache) |
|-------|--------|------------------------|
| Docker-Pull | 20-60 s × N Jobs | <5 s × N Jobs |
| `cargo install cargo-deny` | 60-90 s | 0 s (gecached) |
| `cargo install cargo-llvm-cov` | 120-180 s | 0 s (gecached) |
| `target/`-Dekompression pro Job | 30-60 s | 30-60 s (nur Build/Test/Cov/Interop) |

**Gesamt-Einsparung pro Pipeline (warmer Cache):** ~3-5 Minuten.

## CI/CD-Variablen setzen

Im GitLab-Projekt unter **Settings → CI/CD → Variables**:

| Variable | Wert (Beispiel) | Protected | Masked |
|----------|-----------------|-----------|--------|
| `NEXUS_DOCKER_REGISTRY` | `nexus.amstk.internal:5000` | yes | no |
| `NEXUS_CARGO_INDEX` | `sparse+https://nexus.../repository/crates-io/` | yes | no |

Ohne diese Variablen faellt die Pipeline automatisch auf docker.io +
crates.io zurueck — kein Breaking-Change fuer externe Forks/PRs.

## Nexus-Repository-Setup (Server-Seite)

Im `nexus.amstk.internal`:

1. **Docker Hosted/Proxy Repository** auf Port 5000, proxying docker.io.
2. **Cargo Sparse Index Proxy** pointing auf `https://index.crates.io/`
   (aktiviert in Nexus 3.67+; fuer aeltere Versionen raw-proxy auf
   https://crates.io/api/).

Detaillierte Server-Setup-Anleitung siehe
`IfynaNeu/finanzplanung/CLAUDE.local.md`.

## Follow-ups (nicht in dieser Aenderung)

- **Pre-built CI-Image** mit rustfmt+clippy+cargo-deny+cargo-llvm-cov
  bereits vorinstalliert, analog `CI_ANGULAR_IMAGE` in IfynaNeu. Wuerde
  den Toolchain-Download komplett eliminieren, braucht aber ein Dockerfile
  + Registry-Push-Pipeline.
- **Incremental-Compilation aktivieren** bei Feature-Branches
  (`CARGO_INCREMENTAL=1`). Aktuell workspace-weit auf 0 wegen
  Cache-Konsistenz; koennte per-Job feinjustiert werden.
