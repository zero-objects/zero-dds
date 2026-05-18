# glr2 CI-Integration — Snapshot-Rollback aus Pipelines

Wie aus einer GitLab-Pipeline ein Rollback der Win11-Runner-VM auf
einen bekannten Snapshot getriggert werden kann, ohne SSH-Key.

## Wie es funktioniert

```
┌────────────────────┐
│  GitLab CI Job     │
│  (Linux Runner)    │
│                    │
│  curl POST PVE API │──┐
└────────────────────┘  │  PVEAPIToken (masked)
                        ▼
                ┌───────────────────────┐
                │  pve.sandra-kessler   │
                │  /api2/.../rollback   │
                └────────┬──────────────┘
                         ▼ rollback
                ┌────────────────────┐
                │  VM 114 (glr2)     │
                │  Snapshot active   │
                └────────────────────┘
                         ▲
                         │ Runner kommt
                         │ online
                ┌────────┴───────────┐
                │  Folge-Job mit     │
                │  tags: [windows]   │
                │  laeuft auf glr2   │
                └────────────────────┘
```

## Setup einmalig

### 1. PVE-Token (auf pve als root)

Bereits angelegt, kann re-rotiert werden:

```bash
pveum role add CISnapRoll -privs "VM.Snapshot.Rollback,VM.Audit,VM.PowerMgmt"
pveum user add ci@pve --comment "GitLab CI runner manager"
pveum acl modify /vms/114 -user ci@pve -role CISnapRoll
pveum user token add ci@pve glr-rollback --privsep 0
# → Token-Secret notieren
```

### 2. CI-Variablen (Project-Settings → CI/CD → Variables)

| Variable | Value | Masked |
|----------|-------|--------|
| `PVE_API_URL` | `https://192.168.178.4:8006` | nein |
| `PVE_API_TOKEN_ID` | `ci@pve!glr-rollback` | nein |
| `PVE_API_TOKEN` | `<secret>` | **ja** |
| `GITLAB_API_TOKEN` | `<PAT mit api-scope>` | **ja** |

Bereits gesetzt für fishermen21/zerodds.

### 3. CI-Template einbinden

`.gitlab-ci.yml`:

```yaml
include:
  - local: ci/jobs/windows-runner.yml

stages:
  - prepare
  - build
  - test
```

## Patterns

### Pattern A — on-demand Reset

User triggert Pipeline mit `RESET_WINDOWS_TO=clean-baseline` (oder
`RESET_WINDOWS=true`), der Reset-Job läuft, dann Build:

```yaml
windows-reset-clean:
  extends: .windows-rollback
  # rolllt nur wenn RESET_WINDOWS_TO oder RESET_WINDOWS gesetzt ist

windows-build:
  extends: .windows-clean-build
  script:
    - choco install -y rustup-init
    - rustup-init -y --default-toolchain stable
    - $env:Path += ";$env:USERPROFILE\.cargo\bin"
    - cargo build --release
```

Trigger:
- GitLab-UI → "Run Pipeline" → variable `RESET_WINDOWS_TO=clean-baseline`
- Oder via API: `curl -X POST -F "variables[RESET_WINDOWS_TO]=clean-baseline" .../trigger/pipeline`

### Pattern B — Profil-Snapshot (Tools schon drin)

Tools sind im Snapshot eingebrannt. Build nutzt direkt das Profil:

```yaml
prepare-zerodds-windows:
  extends: .windows-rollback
  rules:
    - if: '$ROLLBACK_PROFILE != null'
      when: on_success
  variables:
    GLR2_TARGET_SNAP: "$ROLLBACK_PROFILE"

build-zerodds-windows:
  extends: .windows-profile
  needs:
    - job: prepare-zerodds-windows
      optional: true
  script:
    # Tauri-CLI ist im snapshot zerodds-windows schon installiert
    - cargo tauri build
```

### Pattern C — kein Reset, einfach laufen lassen

Default: gar nichts resetten, der Runner ist im aktuellen Stand.
Tagged-Job läuft auf glr2:

```yaml
windows-smoke:
  stage: lint
  tags: [windows]
  script:
    - powershell -Command "Get-Host"
```

Bei Bedarf manuell rollback'n via:

```bash
ssh root@pve glr-snapshot rollback 114 clean-baseline
```

## Workflow: neuen Profil-Snapshot bauen

```bash
# 1) Auf clean-baseline zurueck
ssh root@pve glr-snapshot rollback 114 clean-baseline

# 2) Im Win11 (RDP/noVNC): Tools installieren
#    z.B. choco install -y rustup-init visualstudio2022buildtools wix
#    cargo install tauri-cli

# 3) Snapshot mit dem neuen Profil
ssh root@pve glr-snapshot save 114 zerodds-windows \
  "+ Tauri 2.0 CLI + MSVC 2022 + WiX"

# 4) Pipeline triggern mit RESET_WINDOWS_TO=zerodds-windows wenn gewollt
```

## Permissions-Bound

Der `ci@pve!glr-rollback` Token kann **nur**:
- Snapshot-Rollback auf VM 114 (`VM.Snapshot.Rollback`)
- VM-Status lesen (`VM.Audit`)
- VM start/stop (`VM.PowerMgmt`)

Er kann **nicht**:
- Neue VMs erstellen
- Snapshots erstellen oder löschen
- VM-Config ändern
- Auf andere VMs zugreifen

Bei Token-Leak ist der Schaden begrenzt auf "VM 114 Rollback-Spam".

## Debug

| Symptom | Check |
|---------|-------|
| Pipeline-Job hängt im Reset | PVE-API erreichbar? `curl -sk $PVE_API_URL/api2/json/version` aus dem Job-Container |
| Rollback returnt 403 | Token-ACL: `pveum acl list \| grep ci@pve` |
| Rollback OK aber Runner kommt nicht online | VM-Boot-Time? `qm status 114` + `glr-snapshot list 114` |
| 404 beim Rollback-Call | Snapshot existiert nicht: `glr-snapshot list 114` |
| Self-signed-cert error | `curl -k` (insecure) — Template macht das schon |
