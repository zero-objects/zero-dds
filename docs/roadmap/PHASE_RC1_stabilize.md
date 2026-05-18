# Phase RC1-stabilize

**Goal:** sauberen `1.0.0-rc.1`-Tag aus dem Workspace fahren mit allen
Stage-1-Distro-Channels live und ohne known-broken Findings.

**Status:** 🔄 in-progress

## In-Scope

- [x] 50 Spec-Coverage-Docs auf 0/0 partial/open für die RC1-Layer (modulo
      8 declared partials in idl-4.2 long-double + xcdr2-bindings V-3..V-12)
- [x] crates.io Publish-Vorbereitung (97 Crates auf `1.0.0-rc.1`,
      publish-flag, version-bump, pflichtfelder, dev-mirror sync)
- [x] GitHub-Repo-Push (`zero-objects/zero-dds`)
- [x] CI-Config + GitHub-Actions release.yml
- [x] Stage-1-Distro-Channels: 12/12 live (siehe user-guide roadmap-table)
- [x] Website live auf zerodds.de + .org + spawning.de mit dual-stack TLS
- [x] Public-Mirror sauber (forbidden-token-frei, github/-Tree-only)
- [x] alle Internal-Links auf der Website grün, alle GitHub-Refs valide
- [x] Vendor-ID-pending-Disclaimer auf Landing + Claims
- [ ] `cargo login` + `cargo publish` über alle 97 Crates in DAG-Order
- [ ] Workspace-Tag `git tag 1.0.0-rc.1 && git push --tags`
- [ ] Auto-PRs an Homebrew-Tap + Scoop-Bucket greifen + grün
- [ ] AUR auto-update Job läuft sauber durch
- [ ] APT/RPM-Repo zeigt rc.1-Pakete + Cron-Pull-Deploy synct
- [ ] Smoke-Test: `apt install zerodds`, `brew install`, `scoop install`
      auf jeweils einer frischen VM

## Out-of-Scope (für rc.1)

- Datalake-Engine — schiebt RC2
- AMQP 0.9 — schiebt RC2
- Demo + Tutorial Audit — schiebt RC2
- Micro-Profile-Audit — schiebt RC2
- Sprach-Bindings über Core-9 hinaus — RC3
- News-Sektion auf der Website — wartet auf 1.0-final + OMG-Vendor-ID

## Acceptance

1. `cargo install dds-dcps` auf einer leeren Rust-Toolchain funktioniert
2. `apt install zerodds` (Debian 12 frische VM) installiert + bridge-Daemon
   startet
3. `brew install zero-objects/homebrew-zerodds/zerodds` (macOS 14 frische
   VM) installiert + bridge-Daemon startet
4. GitHub-Releases-Page zeigt 8 cross-platform Binaries + AppImage,
   minisign-signed
5. zerodds.org/.de zeigen alle 217+ links grün, OMG-Vendor-ID-Banner
   sichtbar

## Estimate

Tage bis Wochen — die meisten Items sind Trigger-pull (cargo login,
git tag), nicht neu zu bauen.

## Dependencies

Keine externen — alles nur noch ausführen.

## Risks

- **Cargo-publish DAG**: 97 Crates mit 60s rate-limit ≈ 100 min, ein
  fail-mid-publish hinterlässt halb-published State. Mitigation: dry-run
  der ersten 10 Crates + monitoring tail.
- **Auto-PRs zu Brew/Scoop-Tap**: GitHub-Actions-Token hat begrenzte
  Lebenszeit. Mitigation: vor dem Tag-Push Token-Health prüfen.
