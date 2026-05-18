# glr2 — Windows GitLab Runner auf pve

Snapshot-basiertes Setup für einen Windows-Build-Runner auf
Proxmox VE 9.1, der zwischen Projekt-Profilen rollback'n kann.

## VM-Spec

| Item | Wert |
|------|------|
| VMID | 114 |
| Name | glr2 |
| OS  | Windows 11 25H2 DE |
| RAM | 16 GB (kein Ballooning) |
| Cores | 8 (host CPU passthrough) |
| Disk | 200 GB virtio-scsi auf `tank` (ZFS, cache=writeback, ssd=1, discard=on, iothread=1) |
| BIOS | OVMF UEFI + TPM 2.0 + secure-boot (pre-enrolled-keys) |
| Net | virtio bridge=vmbr0 firewall=1 |
| Machine | pc-q35-10.1 (PVE-default für Win11) |

ISOs:
* Setup: `fasttanksother:iso/Win11_25H2_de.iso`
* virtio-Treiber: `tankother:iso/virtio-win-0.1.271.iso`

## OS-Install (im PVE-noVNC)

1. Browser → https://pve.sandra-kessler.eu:8006 → VM 114 → "Console"
2. Bei "Wo möchten Sie Windows installieren?":
   - "Treiber laden" → `D:\amd64\w11\` → **Red Hat VirtIO SCSI controller**
   - 200 GB erscheinen
3. Optional auch laden: `D:\NetKVM\w11\amd64\` (Net), `D:\Balloon\w11\amd64\` (Memory)
4. Win-Setup durchziehen → erster Boot
5. Auf D: (virtio-ISO) → **`virtio-win-guest-tools.exe`** ausführen
   → installiert qemu-guest-agent + restliche Treiber
6. Windows-Updates ziehen (lange — typisch 30+ min)

## GitLab Runner Install

Nach dem Reboot post-Updates:

```powershell
# Als Admin in PowerShell:
$arch = "amd64"
$installer = "$env:TEMP\gitlab-runner.exe"
Invoke-WebRequest `
  "https://gitlab-runner-downloads.s3.amazonaws.com/latest/binaries/gitlab-runner-windows-${arch}.exe" `
  -OutFile $installer
mkdir C:\GitLab-Runner -Force
Move-Item $installer C:\GitLab-Runner\gitlab-runner.exe -Force

# Service installieren als Local System (für Headless-Build)
cd C:\GitLab-Runner
.\gitlab-runner.exe install
.\gitlab-runner.exe start

# Registrieren — Token aus Memory `reference_gitlab_token.md`
# (claude_all). Tag: windows
.\gitlab-runner.exe register `
  --non-interactive `
  --url "https://gitlab.sandra-kessler.eu/" `
  --token "<runner-registration-token-aus-gitlab-UI>" `
  --executor "shell" `
  --shell "powershell" `
  --description "glr2 windows-runner" `
  --tag-list "windows,glr2"
```

Registration-Token gibt's im GitLab unter:
- Project → Settings → CI/CD → Runners → "New project runner"
- ODER Group/Instance-Runner für mehrere Projekte

## Snapshot-Strategie

Auf pve als root:

```bash
# Nach OS-Install + Updates + Runner-Install:
glr-snapshot save 114 clean-baseline "Win11 25H2 + Updates + GitLab Runner"

# Pro Projekt:
glr-snapshot rollback 114 clean-baseline   # zurück zur baseline
# Im Win: projekt-spezifische Tools installieren (z.B. Tauri CLI, MSVC)
glr-snapshot save 114 zerodds-windows "+ Tauri 2.0 CLI + MSVC 2022 + vcpkg"

# Switch auf anderes Projekt:
glr-snapshot rollback 114 clean-baseline
# tools für project-X
glr-snapshot save 114 project-X-windows "+ project-X SDK + ..."

# Liste aller Snapshots:
glr-snapshot list 114
```

Aktuelle Profile-Konvention:

| Snapshot | Inhalt |
|----------|--------|
| `fresh-install` | Win11 25H2 DE installiert + Updates, lokales Konto, kein Runner |
| `clean-baseline` | + virtio-tools + GitLab Runner #4 registriert (tags: `windows,glr2`) |
| `zerodds-windows` | + Rust 1.95 MSVC + Tauri-CLI 2.11 + VS BuildTools 2022 + WiX + NSIS + Node 24 + Git + cmake/ninja/llvm |
| `project-X-windows` | + project-X-Toolchain |

`zerodds-windows` Env-Vars (Machine-scope):

* `PATH` enthält `C:\Windows\System32\config\systemprofile\.cargo\bin`
* `CARGO_HOME=C:\Windows\System32\config\systemprofile\.cargo`
* `RUSTUP_HOME=C:\Windows\System32\config\systemprofile\.rustup`

Cargo findet `link.exe` via `vswhere.exe` (kein vcvars-Sourcing nötig).

## Token-Handling

Damit der Runner-Token NICHT durch Snapshot-Rollbacks invalidiert
wird:

* **Variant A (einfach)**: Token bleibt im Snapshot. Bei Rollback ist
  derselbe Token wieder aktiv. GitLab sieht dann zwei Connection-
  Versionen — alter aufmachen, der neue meldet sich. Funktioniert
  meistens, kann aber zu "ghost-runner" auf GitLab-Seite führen.

* **Variant B (sauber)**: Daten-Disk außerhalb des Snapshots.
  ```bash
  # Zweite vdisk anhängen, NICHT in Snapshots:
  qm set 114 --scsi1 tank:5,backup=0
  # In Win11 als E: formatieren, Runner-Config dort ablegen:
  #   C:\GitLab-Runner\config.toml → Symlink nach E:\runner\config.toml
  # Dann sind Token + State persistent über Rollbacks.
  ```
  Die `backup=0`-Flag exclude'd die Disk aus snapshots.

## Reset-Recipe

Wenn glr2 in einen unbekannten State gerät:

```bash
glr-snapshot rollback 114 clean-baseline
# Runner re-registrieren wenn nötig
```

Wenn `clean-baseline` selbst beschädigt ist:

```bash
# vzdump-Backup wiederherstellen:
qm restore 114 /path/to/vzdump-qemu-114-*.vma.zst --force 1
```

## Routine: Wöchentliches Backup

```bash
# In /etc/cron.weekly/:
vzdump 114 --storage fasttanksother --mode snapshot --compress zstd
```

## Debug

| Symptom | Check |
|---------|-------|
| Win-Setup sieht keine Disk | virtio-scsi-Treiber nicht geladen → "Treiber laden" → `D:\amd64\w11\` |
| Net funktioniert nicht | NetKVM-Treiber nicht installiert → `D:\NetKVM\w11\amd64\netkvm.inf` |
| Runner offline in GitLab | Service-Status `Get-Service gitlab-runner`; Logs in `C:\GitLab-Runner\config.toml` directory |
| Snapshot-Rollback hängt | `qm shutdown 114 --timeout 60` reicht nicht; `qm stop 114` (hard) |
| TPM-Fehler im Boot | `/tank/vm-114-disk-2` (tpmstate) prüfen — wurde vom swtpm beim ersten Start manufactured |
