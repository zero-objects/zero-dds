<!--
SPDX-License-Identifier: Apache-2.0
Copyright 2026 ZeroDDS Contributors
-->

# ZeroDDS Release Playbook (rc.2 → rc.3 and beyond)

End-to-end procedure for cutting a ZeroDDS release: version-flagging the
workspace, publishing to crates.io, mirroring into the public GitHub repo,
running the GitHub release pipelines, and pushing the documentation live.

This is the **executable spec** of "what ships". It encodes the lessons from the
rc.2 incident (tag/SHA divergence) as hard guardrails. Read the whole thing once
before running any step — several phases are hard to reverse.

> Scope note: this playbook is the canonical superset. The older
> [`cargo-publish.md`](cargo-publish.md) covers only the crates.io leg and is
> still accurate for that phase.

---

## 0. Topology you must hold in your head

There are **two repos**, deliberately separate:

| | GitLab root (private) | GitHub mirror (public) |
|---|---|---|
| Path | `/Users/sandrakessler/projects/zerodds` (worktrees: `…/zerodds-deen`) | `/Users/sandrakessler/projects/zerodds/github` |
| Remote | `origin` → gitlab.sandra-kessler.eu | `origin` → github.com/zero-objects/zero-dds |
| Tracked from root? | — | **No** — `/github/` is gitignored (own `.git`) |
| `.github/workflows` | inert (root uses `.gitlab-ci.yml`) | **active** — release.yml + publish-*.yml run here |
| `.gitlab-ci.yml` | active | excluded from mirror |
| Sync direction | source of truth | add-only copy from root, **manual** |

**Current state (verified 2026-06-17):**

- Root `[workspace.package] version` = **`1.0.0-rc.3`** (already bumped).
- Root tags: only **`v1.0.0-rc.1`** (rc.2 was *never* tagged on root).
- Mirror `Cargo.toml` version = **`1.0.0-rc.2`**; mirror tags: `v1.0.0-rc.1`, `v1.0.0-rc.2`.
- **`crates/routing-service` is NOT yet in the mirror** (new since rc.2).
- `cargo-dag` reports **121 publishable crates** (rc.1 published 97 → +24 new,
  incl. opcua-pubsub, mqtt-broker, amqp, durability-*, routing-service @ #102).

### The rc.2 post-mortem (why guardrails exist)

rc.2 was cut **github-first**: the mirror was synced from a state that did not
match the root, then tagged `v1.0.0-rc.2` there — so the same tag name pointed at
**different SHAs / different content** on the two repos, and rc.2 never landed on
root at all. Root then jumped rc.1 → rc.3 in `Cargo.toml`, skipping a root rc.2.

**Guardrails derived from this (do not skip):**

1. **Root is source of truth. Always publish crates.io from root, sync the
   mirror from root, tag the *synced* mirror content — never the reverse.**
2. **A tag name must map to byte-identical content on both repos.** After
   syncing, diff the trees before tagging (Phase C.4).
3. **Bump the version in lock-step.** Root and mirror `Cargo.toml` must both read
   `1.0.0-rc.3` before either is tagged.

---

## 1. Pre-flight (root, before anything)

Everything the CI enforces must already be green on root `main`:

```bash
cd /Users/sandrakessler/projects/zerodds      # or your clean worktree on main
git fetch origin && git switch main && git pull --ff-only

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace        # note: zerodds-durability-service-bin needs a
                              # system libduckdb to LINK; install it or
                              # --exclude that one bin on a host without it
cargo deny check licenses     # license audit (publish gate)
```

Confirm the GitLab pipeline for the `main` HEAD is green at **job level** (not
just the top-level badge — see memory "Pipeline-Status auf Job-Level prüfen").

---

## 2. Phase A — Flag the crates as rc.3

The workspace version is already `1.0.0-rc.3`. The **open problem** is dependency
drift: inter-crate `version = "…"` requirements were never normalized.

```bash
# Drift audit:
grep -rhoE 'version = "1\.0\.0-rc\.[0-9]"' crates/*/Cargo.toml | sort | uniq -c
#   → 233 × rc.1, 34 × rc.2, 0 × rc.3   (as of 2026-06-17)
```

**Why it still resolves:** a caret requirement with a pre-release
(`^1.0.0-rc.1`) matches any `1.0.0-rc.N` with `N ≥ 1` (same `1.0.0` base). So
rc.1/rc.2 requirements forward-match the rc.3 crates we publish. It is **not
broken**, but it is the exact inconsistency that hides divergence bugs.

**Recommended: normalize everything to rc.3** (one mechanical commit). Use
`cargo-edit`'s `set-version`, which bumps both `[package] version` *and* every
intra-workspace dependency requirement:

```bash
cargo install cargo-edit          # provides `cargo set-version`
cargo set-version --workspace 1.0.0-rc.3
# Verify zero drift afterwards:
grep -rhoE 'version = "1\.0\.0-rc\.[0-9]"' crates/*/Cargo.toml | sort | uniq -c
#   → expect only rc.3
cargo update --workspace          # refresh Cargo.lock
git add -A && git commit -m "chore(release): normalize all crate deps to 1.0.0-rc.3"
```

> Alternative: `cargo workspaces version` (from `cargo-workspaces`) — also valid;
> pick one and stay consistent. **Do not hand-sed** 267 requirement lines.

**Crate-specific note:** `crates/routing-service/Cargo.toml` was authored with
`rc.2` dep requirements — `set-version` fixes it along with the rest.

Re-run the Phase 1 gates after the bump (`cargo build --workspace`, fmt/clippy).
Then push the bump to root `main` and let the GitLab pipeline confirm green.

---

## 3. Phase B — Publish to crates.io (from root)

crates.io publish is **manual** (it is *not* in `release.yml`). It is driven by
`tools/cargo-dag` (topological sort) and an idempotent shell loop.

### B.1 Generate the publish order

```bash
cd /Users/sandrakessler/projects/zerodds
cargo run -q -p zerodds-cargo-dag -- . --only-publishable --format flat \
  > .publish-rc3.order
wc -l .publish-rc3.order        # → 121 (incl. zerodds-routing-service @ ~#102)
```

`--only-publishable` drops the 10 `publish = false` crates (lint, cpp,
safe-crates-only, corba-interop, cargo-dag, and the 5 fuzz crates). Kahn's
algorithm, alphabetical tie-break → deterministic order.

### B.2 Dry-run the whole set first

```bash
while IFS= read -r c; do
  echo "=== $c ==="
  cargo publish -p "$c" --dry-run --allow-dirty --no-verify || break
done < .publish-rc3.order
```

`--no-verify` checks packaging + metadata without a full rebuild (a full
`--dry-run` verify rebuild fails for downstream crates because their rc.3 deps
aren't on crates.io yet — expected, not a real error). The metadata gate is what
matters: every public crate needs `description`, `license`, `repository`,
`homepage`, `readme`, `keywords`, `categories`, `publish = true`
(template: [`crate-readme-template.md`](crate-readme-template.md)).

### B.3 The real publish loop (`.publish-rc3.sh`)

Copy `.publish-rc1.sh` → `.publish-rc3.sh` and change `VERSION="1.0.0-rc.3"` /
`LOG=.publish-rc3.log` / `ORDER=.publish-rc3.order`. It is **idempotent** — it
skips any crate whose rc.3 already answers `200` on the crates.io API, so it is
safe to re-run after a rate-limit stall.

```bash
#!/usr/bin/env bash
set -u
LOG=.publish-rc3.log; ORDER=.publish-rc3.order; VERSION="1.0.0-rc.3"
cargo run -q -p zerodds-cargo-dag -- . --only-publishable --format flat > "$ORDER" 2>>"$LOG"
total=$(wc -l < "$ORDER" | tr -d ' '); i=0
while IFS= read -r crate; do
  i=$((i+1))
  http=$(curl -s -o /dev/null -w "%{http_code}" \
    "https://crates.io/api/v1/crates/$crate/$VERSION" 2>/dev/null || echo 000)
  [ "$http" = "200" ] && { echo "($i/$total) SKIP $crate"; continue; }
  echo "($i/$total) $crate" | tee -a "$LOG"
  out=$(cargo publish -p "$crate" --allow-dirty 2>&1); rc=$?
  echo "$out" >> "$LOG"
  if [ $rc -ne 0 ] && echo "$out" | grep -qiE 'rate.?limit|429|too many'; then
    echo "RATE-LIMIT — sleep 600"; sleep 600
    cargo publish -p "$crate" --allow-dirty >> "$LOG" 2>&1; rc=$?
  fi
  [ $rc -ne 0 ] && { echo "FAILED $crate (rc=$rc) — stop"; exit 1; }
  sleep 60      # crates.io steady-state rate limit
done < "$ORDER"
```

```bash
cargo login <crates.io-token>      # one-time per host
chmod +x .publish-rc3.sh && ./.publish-rc3.sh
```

121 crates × ~60 s ≈ **2 h minimum** (longer with rate-limit backoffs). Run it
in a persistent session. `.publish-rc3.{sh,order,log}` are gitignored — they are
the "internal rc flag" artifacts, never committed.

### B.3.1 Gotchas encountered (rc.3 run — fix in pre-flight next time)

Two real blockers stopped the loop mid-run; both are now understood:

1. **`cargo-dag` does not order dev-dependencies.** A crate with a *versioned*
   intra-workspace **dev-dependency** on a crate published *later* in the order
   fails to package: `failed to select a version for the requirement
   '<dep> = "^1.0.0-rc.3"' … candidate versions found: 1.0.0-rc.1` (the rc.3 dep
   isn't on crates.io yet, and `--no-verify` does **not** help — the resolution
   happens during *packaging*, not verify). **Fix:** keep intra-workspace
   dev-deps **path-only** (`{ path = "../x" }`, no `version`) — cargo drops them
   on publish, so they impose no ordering constraint. The core crates already do
   this; only `zerodds-secure-permissions` carried a versioned one. Pre-flight
   scan:
   ```bash
   # flag versioned intra-workspace dev-deps whose target is published later
   grep -rEl 'zerodds-[a-z0-9-]+ *= *\{[^}]*version[^}]*path' crates/*/Cargo.toml tools/*/Cargo.toml
   ```

2. **The `libduckdb` binary fails verify-link on a host without libduckdb.**
   `zerodds-durability-service-bin` links `-lduckdb`; the publish *verify build*
   links the final binary → `ld: library 'duckdb' not found`. The lakehouse
   *lib* publishes fine (libs only emit `.rlib`, no link). **Fix:** publish that
   one binary with `cargo publish -p zerodds-durability-service-bin --no-verify`
   (CI already verified it with libduckdb), or install libduckdb on the host
   (§9.5). This is the only crate that needs it.

### B.4 Rollback

A bad crate cannot be deleted, only yanked:

```bash
cargo yank --version 1.0.0-rc.3 <crate-name>          # hide
cargo yank --version 1.0.0-rc.3 --undo <crate-name>   # un-hide
```

docs.rs builds **automatically** from each crates.io publish — no action; just
verify a sample (`https://docs.rs/zerodds-routing-service/1.0.0-rc.3`) afterward.

---

## 4. Phase C — Sync the GitHub mirror ("stuff nach github ordner kopieren")

There is **no sync script in this repo yet** (unlike dry-cleaner's
`scripts/build-release.sh`). It has been a manual add-only `rsync`. **Action
item:** create `scripts/build-github-mirror.sh` modeled on dry-cleaner's, so the
"what ships publicly" set is an executable whitelist, not tribal knowledge. Until
then, follow this exactly.

### C.1 Add-only rsync from root → mirror

```bash
cd /Users/sandrakessler/projects/zerodds
rsync -a \
  --exclude '.git/' --exclude 'target/' --exclude 'buildroot/' \
  --exclude 'ci/' --exclude 'diagrams/' \
  --exclude 'website/' --exclude '.gitlab-ci.yml' \
  --exclude '.publish-rc*' --exclude '.dryrun-rc*' \
  crates docs tools man packaging pkg meta-zerodds \
  Cargo.toml Cargo.lock README.md CHANGELOG.md LICENSE NOTICE \
  CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md deny.toml \
  rust-toolchain.toml rustfmt.toml \
  github/
```

Add-only (no `--delete`) is deliberate — never let a sync wipe mirror-curated
files. **`crates/routing-service` is new — confirm it landed in the mirror.**

> **rc.3 trap — directory sources take NO trailing slash, dest does.** Under
> zsh an unquoted `$SRC` variable is **not** word-split (one bogus path → rsync
> dies); pass the sources as literal args, not a variable. And `crates/`
> (trailing slash) copies *contents* into `github/`, flattening the tree — use
> `crates` (no slash) so it maps to `github/crates`.

> **rc.3 trap — DO NOT exclude `benches/` or `examples/`.** 12 manifests declare
> explicit `[[bench]]`/`[[example]]` targets (bench-suite's Wave-4 benches,
> dcps examples, …). Excluding the `.rs` source makes cargo abort with
> *"can't find bench"* before clippy/cargo-deny even start — the mirror CI goes
> red on a manifest-parse error. These dirs are crate source shipped to
> crates.io, not heavy fixtures (<300 KB total). After any sync, verify every
> declared target file exists (script in C.4). Cost rc.3 a red CI + two extra
> mirror commits.

> **rc.3 trap — strip stale C# build artifacts.** The mirror had `crates/cs/
> csharp/**/bin|obj` (DLLs/pdb, rc.2 `.nuspec`) tracked and *not* gitignored.
> They re-stage on every `git add -A` and would ship publicly. Add
> `crates/cs/csharp/**/bin/` + `**/obj/` to the mirror `.gitignore` and
> `git rm --cached` them once.

### C.2 Apply the mirror patches

The mirror differs from root in a few curated ways (see the rc.2 sync commit
`31dece7`): author scrubbing in the root `Cargo.toml`, README badge GitLab→GitHub
Actions, internal-link drops, rulesets marked `linguist-generated`. Re-apply
these (eventually script them in C's action item).

### C.3 Bump the mirror version to rc.3

```bash
cd github
grep -m1 'version = ' Cargo.toml      # currently 1.0.0-rc.2
cargo set-version --workspace 1.0.0-rc.3   # match root exactly
```

### C.4 Guardrail — verify content parity BEFORE tagging

This is the step rc.2 skipped. The tagged mirror content must equal the root
content for the same files:

```bash
# from repo root; compare the shipped subset (ignore mirror-only patches)
diff -rq --exclude='.git' crates  github/crates   | grep -v 'routing-service' || echo "crates parity OK"
diff -q Cargo.toml github/Cargo.toml || echo "expected: only author-scrub + version lines differ"
```

Investigate every unexpected diff. Only proceed when the mirror is a faithful
rc.3 of root.

**Also verify every declared cargo target file exists** (this is what catches
the benches/examples exclude trap above — `cargo metadata` does *not* check
target paths, only `clippy --all-targets` does, i.e. only the mirror CI):

```bash
cd github && python3 - <<'PY'
import re, pathlib
miss=[]
for ct in list(pathlib.Path('.').glob('crates/*/Cargo.toml'))+list(pathlib.Path('.').glob('tools/*/Cargo.toml')):
    base=ct.parent; txt=ct.read_text()
    for m in re.finditer(r'path\s*=\s*"([^"]+\.rs)"', txt):
        if not (base/m.group(1)).exists(): miss.append(f"{base}/{m.group(1)}")
    for sect,sub in [('bench','benches'),('example','examples'),('bin','src/bin')]:
        for blk in re.findall(r'\[\[%s\]\](.*?)(?=\n\[|\Z)'%sect, txt, re.S):
            if 'path' in blk: continue
            nm=re.search(r'name\s*=\s*"([^"]+)"', blk)
            if nm and not (base/sub/(nm.group(1)+'.rs')).exists() and not (base/sub/nm.group(1)/'main.rs').exists():
                miss.append(f"{base}/{sub}/{nm.group(1)}.rs (default {sect})")
print("MISSING:\n  "+"\n  ".join(miss) if miss else "OK: all declared target files present")
PY
```

### C.5 Verify the staged tree builds standalone

The mirror must build for an outside contributor (deps from crates.io, not
workspace paths):

```bash
cd github
cargo build --workspace
cargo test  --workspace        # or --exclude the libduckdb bin
cd ..
```

### C.6 Commit + push the mirror (content first, tag later)

```bash
cd github
git add -A
git commit -m "release: sync from root + bump workspace to 1.0.0-rc.3"
git push origin main
```

**Do not tag yet** — tag only after the GitHub *content* push succeeds, so the
tag lands on the pushed SHA (Phase D).

---

## 5. Phase D — GitHub release commit, tag & pipelines

### D.1 Adjust the GitHub pipelines if needed

Review `github/.github/workflows/` against this release:

- **`release.yml`** — cargo-dist multi-target build (Linux gnu/musl x86_64+aarch64,
  macOS, Windows) → deb/rpm/docker/msi packages → homebrew/aur/scoop taps →
  `github-release` (with the **draft→published** poll workaround for softprops
  rate-limit). Triggered on tag `v[0-9]+.[0-9]+.[0-9]+*` or `workflow_dispatch`.
  Needs signing secrets (minisign, Apple notarization, optional Windows cert).
- **`publish-{npm,pypi,maven,nuget,deb,rpm}.yml`** — callable. npm/pypi use OIDC
  **Trusted Publishing** (no tokens); maven needs Sonatype + GPG secrets; nuget
  needs `NUGET_API_KEY`.
- **`ci.yml`** — fmt/clippy/deny/build-test/no-std/coverage.

If a **new crate** changed the package surface (e.g. routing-service adds a new
binary `zerodds-router`), check whether cargo-dist `[workspace.metadata.dist]`
should ship it as a release artifact, and whether deb/rpm package manifests need
the new binary. **Simulate before pushing** (Section 7).

### D.2 Tag the mirror (the version flag commit)

```bash
cd github
git tag v1.0.0-rc.3            # on the SHA you just pushed in C.6
git push origin v1.0.0-rc.3    # → triggers release.yml
```

The tag push is the "github release commit mit internem rc.3 flag". `release.yml`
builds all platforms and creates the GitHub Release; the language `publish-*`
workflows fire as configured.

### D.3 Optional: tag root for parity

To avoid the rc.1-style divergence, also tag root at the bump commit so the
version flag exists symmetrically (root has no GitHub Actions, so no pipeline
fires — it is purely a provenance marker):

```bash
cd /Users/sandrakessler/projects/zerodds-deen
git tag v1.0.0-rc.3 && git push origin v1.0.0-rc.3
```

### D.4 Watch the GitHub release run

Use `gh` (or the Actions UI). The `github-release` job's draft→published step
self-heals rate-limit draft states. Confirm: release published, all platform
assets + `SHA512SUMS` attached, `prerelease` flag set (rc = prerelease).

### D.5 rc.3 trap — a system-lib binary breaks EVERY workspace build path

`zerodds-durability-svc` links the **system** libduckdb (lakehouse adapter; the
`bundled` source build was dropped — it OOM-killed the runner + added ~1h). A
distro/portable artifact cannot carry a system-libduckdb dependency, so every
build path that compiles the whole workspace fails with `cannot find -lduckdb`.
`release.yml` has **five independent** such paths, and fixing one does not fix
the others — they failed one cycle at a time (≈4 re-tag rounds). Fix them all in
one commit:

| Path | File | Fix |
|---|---|---|
| ci.yml build-test/coverage | `.github/workflows/ci.yml` | install libduckdb (x86_64) + `--exclude` the duckdb crates on aarch64 cross |
| cargo-dist build | root `Cargo.toml` `[workspace.metadata.dist]` | `precise-builds = true` (builds `--package=<app>` per dist app, not `--workspace`) + `dist = false` on the bin |
| .deb | `.github/workflows/publish-deb.yml` + `packaging/linux/deb/publish-deb.yml` | `--exclude zerodds-durability-store-lakehouse --exclude zerodds-durability-service-bin` on the `cargo build --workspace` |
| .rpm | `packaging/linux/rpm/zerodds.spec` | same `--exclude` on `%build`; **and** `%files` must list every new bin/man/unit or rpmbuild fails *"Installed (but unpackaged)"* — skip the durability unit in `%install` |
| AUR | `packaging/linux/arch/PKGBUILD` + `packaging/github-actions/aur-publish.sh` | same `--exclude` |

`dist = false` alone is **not enough** — it only drops the *archive*, cargo-dist
still `--workspace`-builds the bin. Docker images are safe (they build per
`--bin`/`-p`, not `--workspace`). Before re-tagging, grep exhaustively:
`grep -rn 'cargo build.*--workspace' packaging/ .github/workflows/` and verify
the rpm `%files` covers every installed bin/man/unit (script in C.4 style).
Docker release jobs are slow (no inter-run layer cache) but green — don't
mistake slow for hung. The `cli` image is the worst: it builds ~18 bins for
`linux/amd64,linux/arm64`, and the arm64 half runs under QEMU emulation
(~10–50× slower) — it took **~2.5 h** in the rc.3 run and still passed. It
gates `github-release` (which needs `package-docker == success`), so the whole
release waits on it. If that's too slow, the perf followup is per-image
`platforms` (cli → amd64-only) or native arm64 runners (`ubuntu-24.04-arm` +
digest-merge) — but for a release, just let it run; it does finish.

---

## 6. Phase E — Push the documentation live

Three doc surfaces; only the website is a manual deploy.

| Surface | How | Action |
|---|---|---|
| **docs.rs** | auto from crates.io | none — verify a sample URL |
| **Sphinx Python API** | CI artifact (`crates/py/docs`) | not live; QA only |
| **zerodds.de/.org website** | manual `pct push` to LXC 220 @ nr3 | **this section** |

### E.1 Regenerate the release-volatile website content

```bash
cd /Users/sandrakessler/projects/zerodds/website
python3 _tools/sync_release_values.py --write   # version + per-crate stats → _data/release.json
python3 _tools/render_man_pages.py              # man/man1/*.1 → topics/man-pages/
python3 _tools/regen_translation_js.py          # {de,en}.json → {de,en}.js
python3 _tools/sync_fragments.py                # shared chrome
```

If routing-service ships a CLI, add/refresh its man page (`man/man1/zerodds-router.1`)
and re-render. Body stays English; only chrome is bilingual.

### E.2 Link-checker gate (hard)

```bash
python3 _tools/gen_links_report.py              # writes website/links.html
```

Two non-negotiables (see memory): **no broken internal links**, and **no internal
hostnames** (`codepit, pivot, glr1, glr2, nr3, pve`) anywhere in published HTML.
Review `links.html`; resolve every new 404 / unreviewed row.

### E.3 Local preview + user review (mandatory gate)

```bash
cd /Users/sandrakessler/projects/zerodds/website && python3 -m http.server 8000
# open http://localhost:8000 — check version, man pages, translations, links
```

**Rule (memory `feedback_no_direct_live_deploy`): never `pct push` to the live
container without a local preview AND explicit user sign-off.**

### E.4 Deploy to the live container (only after sign-off)

```bash
# from a host that can reach the Proxmox node nr3:
pct push 220 <local-website-tarball> /var/www/zerodds.de/...   # or the cron pull-deploy on nr3
# then reload the webserver inside the container
```

(`pct push <vmid> <local> <remote>` copies one file; in practice the site is
deployed as a tree via the established nr3 mechanism. Confirm the exact path with
the host owner — do not improvise destructive copies.)

---

## 7. Simulating the GitHub pipelines locally (dry-cleaner / Pipewright)

Before pushing anything to GitHub, dry-run the pipelines. The sibling project
**dry-cleaner** is *Pipewright* — a CI tool that plans and runs GitHub Actions /
GitLab CI locally. The release binary is prebuilt:

```bash
PW=/Users/sandrakessler/projects/dry-cleaner/target/release/pipewright
GHWF=/Users/sandrakessler/projects/zerodds/github/.github/workflows
```

### 7.1 Plan (no Docker) — verified working

```bash
$PW detect $GHWF/release.yml      # → github
$PW plan   $GHWF/ci.yml           # jobs in dependency order + the exact commands
$PW plan   $GHWF/release.yml      # inspect the full release job graph
$PW capabilities $GHWF/release.yml  # feature/portability profile
```

`plan` is the cheapest way to confirm a pipeline edit did what you intended
(job graph, images, step commands) without a runner.

### 7.2 Run (Docker) — execute a job locally

```bash
$PW run $GHWF/ci.yml --job fmt              # one job, read-only mount (default)
$PW run $GHWF/ci.yml --trigger push --ref main
```

Read-only mount by default; `--rw-copy` runs on a throwaway copy. Note: jobs
needing real publish tokens / OIDC won't fully succeed locally — that's expected;
use it to validate build/package steps, not the actual publish.

### 7.3 Dry-run the publishes without touching registries

- **crates.io:** `cargo publish -p <c> --dry-run --no-verify` (Section B.2).
- **npm:** `npm publish --dry-run` — or a local **verdaccio** registry
  (`verdaccio` on `localhost:4873`, `npm publish --registry http://localhost:4873`).
- **pypi:** upload to **TestPyPI** (`twine upload --repository testpypi dist/*`).
- **nuget/maven:** the workflows expose `dry_run` inputs — use them.

### 7.4 Staging-verify pattern (from dry-cleaner)

Even without Pipewright, the dry-cleaner discipline applies: the mirror tree
(Phase C.5) is built + tested standalone before the GitHub push. That single step
catches the most common release break (a crate that only builds inside the
workspace).

---

## 8. One-screen checklist

```
PRE  [ ] root main green (fmt/clippy/test/deny), GitLab pipeline green @ job level
A    [ ] cargo set-version --workspace 1.0.0-rc.3 (deps normalized, 0 rc.1/rc.2 left)
     [ ] bump committed + pushed to root main, pipeline green
B    [ ] cargo-dag → .publish-rc3.order (121 crates, routing-service present)
     [ ] dry-run --no-verify across all crates (metadata OK)
     [ ] cargo login; ./.publish-rc3.sh runs to "DONE" (idempotent)
     [ ] spot-check docs.rs for a sample crate
C    [ ] rsync root → github/ (add-only); routing-service landed
     [ ] mirror patches re-applied; mirror Cargo.toml = 1.0.0-rc.3
     [ ] PARITY DIFF clean (guardrail); standalone build+test green
     [ ] git commit + push github main  (content BEFORE tag)
D    [ ] pipelines reviewed/adjusted; simulated via pipewright plan/run
     [ ] git tag v1.0.0-rc.3 on github + push → release.yml runs
     [ ] (optional) tag root v1.0.0-rc.3 for parity
     [ ] GitHub release published, assets + SHA512SUMS attached, prerelease=true
E    [ ] website release values + man pages + i18n regenerated
     [ ] gen_links_report.py: no 404, no internal hostnames
     [ ] local preview + USER SIGN-OFF
     [ ] pct push to LXC 220 @ nr3; live site verified
```

---

## 9. Open decisions (raise before first rc.3 run)

1. **Normalize deps to rc.3, or keep forward-matching?** Recommended: normalize
   (Phase A) — cheap, removes the rc.2-class footgun. Decide once.
2. **Script the mirror sync?** Strongly recommended (Phase C action item) — port
   dry-cleaner's `scripts/build-release.sh` into `scripts/build-github-mirror.sh`
   so "what ships" is reviewable code.
3. **Tag root for parity (D.3)?** Recommended, to stop rc.1-style divergence.
4. **Does cargo-dist need to ship the new `zerodds-router` binary** (and deb/rpm
   manifests)? Verify in `[workspace.metadata.dist]` before D.
5. **libduckdb on the publish host** — install it, or accept that
   `zerodds-durability-service-bin` is build-excluded locally (CI has it).
