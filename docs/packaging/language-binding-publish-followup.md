# Language-Binding-Registry-Publish (PyPI / npm / Maven / NuGet) — RC3

- **Status**: in-progress (alle 4 CI-Pipelines geschrieben, warten auf Tokens + Erst-Runs)
- **Datum**: 2026-05-17 (eroeffnet) · 2026-05-18 (alle Pipelines + Setup-Howto live)
- **Setup-Anleitung**: [`publishing-setup-howto.md`](publishing-setup-howto.md) — Step-by-step pro Channel
- **Sprint-Kontext**: bei Audit der `/website/bindings/*/` Pages am
  2026-05-17 aufgekommen — die Pages versprachen heute
  `pip install zerodds`, `npm install @zerodds/node`,
  `nuget.org/packages/ZeroDDS` und ein Maven-Projekt. Keiner dieser
  Channels ist published, keine CI-Workflows dafuer, keine
  Roadmap-Notes. Bindings-Pages wurden im selben Pass auf den heute
  ehrlichen Install-Pfad (libzerodds + Source-Build der Language-Glue)
  umgestellt; die Channels werden hiermit als RC3-Arbeit getrackt.
- **Verantwortlich**: open

## Was ist offen

Vier Language-Binding-Registries fuer die End-User-Install-Story
publishen, damit die jeweilige Sprachgemeinschaft sich ZeroDDS via
nativen Paketmanager holen kann (statt aus Source bauen):

| Channel | Paket-Name (geplant) | Inhalt | Build-Pfad heute |
|---|---|---|---|
| **PyPI** | `zerodds` | Python-Wheel mit PyO3-gebundener libzerodds (CPython 3.10-3.13, manylinux + macOS arm64/x86_64 + Windows x86_64) | `crates/py/` via `maturin develop` aus Source |
| **npm** | `@zerodds/node` | N-API Node-Addon, libzerodds als Dep-Binary | `crates/ts-node/` via `cargo build` + `node-pre-gyp` (noch nicht eingerichtet) |
| **Maven Central** | `org.zerodds:zerodds-java` | Pure-Java-Artefakt (org.omg.dds.* + zerodds-Vendor-PSM); kein JNI noetig | `crates/java-omgdds/` Pure-Java-Pfad existiert (per project_k15_java_psm_status: 156 done / 0 partial) |
| **NuGet** | `ZeroDDS` | .NET-Klassenbibliothek mit P/Invoke gegen libzerodds (netstandard2.1 + .NET 8) | `crates/cs/` Source-Build + dotnet pack |

## Warum offen

* RC1 (2026-05-08) hat sich auf **Distribution-Pakete fuer System-Bins**
  konzentriert (apt/.deb, rpm, MSI, Brew, AUR, Docker Hub, GitHub
  Releases) — diese decken die ZeroDDS-Runtime + CLI-Tools fuer
  End-User-Operatoren ab. Language-Binding-Registries decken einen
  anderen User-Typ ab (Anwendungsentwickler), wurden aber im
  Release-Scope nicht beruecksichtigt.
* CI-Capacity in RC1 ging in die 7-Target-Matrix
  (`release.yml`) + Deb-/RPM-Publish-Jobs (`publish-deb.yml`,
  `publish-rpm.yml`). Channel-spezifische Publish-Jobs (PyPI-token,
  npm-token, Sonatype-Maven-creds, NuGet-API-key) wurden nicht
  eingerichtet.
* Im PACKAGING.md (Section 7a + 4) sind die 4 Channels **nicht**
  erwaehnt — die Doku hat den Audit-Hint also nicht aufgefangen.

## Implikationen

* Bindings-Pages mussten ehrlich gemacht werden — die heutigen
  Install-Snippets (`pip install zerodds`, `npm install
  @zerodds/node`, NuGet-Link, Maven-Projekt) verlinkten auf
  nicht-existierende Pakete. Korrigiert im selben Commit als dieser
  Followup angelegt wurde.
* End-User-Onboarding fuer C#/Java/Python/TypeScript-Devs ist
  schwerer als noetig — heute "git clone + cargo build" statt
  `pip install`.
* SEO / Discoverability auf den Registries fehlt komplett (kein
  PyPI/npm/Maven/NuGet Index-Eintrag).

## Wann pick-up sinnvoll

Trigger fuer den RC3-Pickup:

* **PyPI-Wheel zuerst** — die `crates/py/` ist bereits maturin-faehig,
  Workflow ist im Vergleich zum Rest am einfachsten (maturin-action
  publish-Job + PyPI-token via GitHub-Secrets). Empfohlen als
  Quick-Win der Welle.
* **npm-Addon zweites** — N-API-Binding existiert in `crates/ts-node/`,
  braucht `node-pre-gyp` oder `prebuild-install` Verdrahtung +
  Multi-Platform-Tarballs. Mittlerer Aufwand.
* **Maven-Central + NuGet** als dritter Block — beide haben hoehere
  Aufnahme-Buerokratie (Sonatype-Account fuer Maven, MS-Account fuer
  NuGet). Lohnt sich erst wenn ein Enterprise-Use-Case anliegt.

## Implementations-Pfad

Geschaetzte Dauer fuer alle vier zusammen: **2-3 Sprints**, davon:

1. **Sprint 1 (PyPI)** — **in-progress 2026-05-18**
   - ✅ `pyproject.toml` mit `dynamic = ["version"]` (sync mit
     Cargo-workspace-Version), Klassifikator-Liste, Project-URLs.
   - ✅ Lokaler `maturin build --release` baut ein abi3-Wheel
     (`zerodds-1.0.0rc2-cp38-abi3-macosx_11_0_arm64.whl`) das
     installiert + den vollen DCPS-API importiert.
   - ✅ `.github/workflows/publish-pypi.yml` geschrieben — Build-Matrix
     fuer manylinux2014 (x86_64+aarch64) / macOS (arm64+x86_64) /
     Windows (x86_64). PyO3/maturin-action + pypa/gh-action-pypi-publish.
     Test-PyPI- und PyPI-Pfad jeweils gated per workflow_dispatch
     `target_index` Input.
   - 🔜 **User-Action**: PyPI-Account, API-Token erstellen, GitHub
     Secrets `PYPI_API_TOKEN` + `TEST_PYPI_API_TOKEN` setzen.
   - 🔜 Erst-Push via `workflow_dispatch` mit `target_index=testpypi`
     zur Verifikation, dann `target_index=pypi` fuer den echten Upload.
   - 🔜 Nach erfolgreichem Live-Upload: bindings/python/index.html
     Disclaimer rausnehmen, `pip install zerodds` als Default zeigen.

2. **Sprint 2 (npm)** — **pipeline ready 2026-05-18**
   - ✅ `crates/ts-node/package.json` mit `@zerodds/node` ist da
     (Pure-TS-FFI via koffi, **kein** N-API-Native-Build noetig —
     deutlich einfacher als urspruenglich gedacht).
   - ✅ `.github/workflows/publish-npm.yml` geschrieben. Plattform-
     neutral (ein einziger Tarball), version sync aus
     Cargo.toml-workspace via `npm version`, dist-tag-Auswahl
     (`latest`/`beta`/`rc`) per workflow_dispatch.
   - 🔜 **User-Action**: npm-Org `zerodds` reservieren, Token
     mit `@zerodds/*` Scope generieren, GitHub-Secret
     `NPM_AUTH_TOKEN` setzen.
   - 🔜 `workflow_dispatch` mit `dry_run` zur Tarball-Inspektion,
     dann ohne dry_run fuer Live-Publish.
   - 🔜 Nach Live: bindings/typescript-node/index.html Disclaimer
     rausnehmen.

3. **Sprint 3 (Maven Central + NuGet)** — **pipelines ready 2026-05-18**
   - **Maven** (`org.zerodds:omgdds`):
     - ✅ `crates/java-omgdds/java/pom.xml` existiert als pure-Java-Setup
       (Java 17, kein JNI).
     - ✅ `.github/workflows/publish-maven.yml` mit Sonatype-Central-
       Portal-Pfad (modern, replaces nexus-staging), GPG-Sign-Setup,
       version sync per `mvn versions:set`.
     - 🔜 **User-Action**: Sonatype-Namespace `org.zerodds` beantragen
       (DNS TXT verifizieren, 1-3 Tage Approval), GPG-Key generieren,
       4 GitHub-Secrets setzen. Plus `pom.xml` um Maven-Central-
       Pflicht-Metadaten (licenses/scm/developers) + Plugins
       (gpg/source/javadoc/central-publishing) erweitern — die
       Pipeline pruft das und failed mit klarer Meldung.
   - **NuGet** (`ZeroDDS`):
     - ✅ `crates/cs/csharp/ZeroDDS/ZeroDDS.csproj` existiert (.NET 8,
       Multi-Project-Solution mit ZeroDDS, ZeroDDS.Cdr,
       ZeroDDS.Cdr.SourceGenerators).
     - ✅ `.github/workflows/publish-nuget.yml` mit `dotnet pack` pro
       Sub-Projekt, SymbolPackages (snupkg) inklusive, version sync
       per `-p:Version=$WS_VERSION`.
     - 🔜 **User-Action**: nuget.org Account, API-Key mit Glob
       `ZeroDDS*`, GitHub-Secret `NUGET_API_KEY` setzen.
     - 🔜 RC2-Optional: separater `ZeroDDS.Native.Runtime` nupkg mit
       libzerodds als runtime/<rid>/native/ Files pro RID, damit
       NuGet-User keine separate System-Install der C-Lib brauchen.

Pro Channel ist ausserdem das Bindings-Pages-Update zu schreiben:
Schritt 1 von "ehrlicher heute" auf "ehrlich + Paket-Install live"
flippen. Etwa 30 min pro Page sobald der Channel live ist.

## Cross-Refs

* `docs/PACKAGING.md` (heutige Distros-Doku, Section 1-4, 7a)
* `docs/specs/zerodds-deployment-1.0.md` (Deployment-Spec)
* `.github/workflows/publish-deb.yml`, `publish-rpm.yml`,
  `release.yml` (Vorlagen-Pipelines)
* `website/bindings/{python,csharp,java,typescript-node}/index.html`
  (Pages die nach Pickup die Phase-2-Note loswerden koennen)
* MEMORY: `project_rc1_publish_pending.md`,
  `project_rc2_release_drift_followups.md`
