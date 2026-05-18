# Publishing-Setup How-To — Language-Binding-Registries

Step-by-step Anleitung um die vier Language-Binding-Registries
(PyPI, npm, Maven Central, NuGet) live zu schalten. Die CI-Pipelines
liegen bereits in `.github/workflows/` und sind workflow_dispatch-faehig
— dieses Doc deckt nur die einmaligen User-Aktionen ab (Account-Setup,
Token-Generation, GitHub-Secrets).

**Reihenfolge der Welle** (nach Komplexitaet):

1. **PyPI** — schnell, Token sofort, Pipeline ready
2. **NuGet** — schnell, Token sofort, Pipeline ready
3. **npm** — schnell, Token sofort, Pipeline ready, ggf. Org-Reservierung
4. **Maven Central** — langsam wegen Sonatype-Namespace-Approval (1-3 Werktage), GPG-Setup

Tracker: [`language-binding-publish-followup.md`](language-binding-publish-followup.md)

---

## 1 · PyPI (`pip install zerodds`)

**Pipeline:** `.github/workflows/publish-pypi.yml`
**Auth:** Trusted Publisher (OIDC) — **kein API-Token noetig**.
**Aufwand:** 10 min Setup, dann Tag-Push laeuft.

### 1.1 Accounts
* PyPI-Account: https://pypi.org/account/register/
* Test-PyPI-Account (empfohlen fuer Trockenlauf): https://test.pypi.org/account/register/

### 1.2 Pending Publisher auf PyPI anlegen

Da das Projekt `zerodds` noch nicht existiert, brauchen wir einen
**Pending Publisher** (wird beim ersten erfolgreichen Upload zum
echten Publisher).

1. https://pypi.org/manage/account/publishing/ → "Add a new pending publisher"
2. GitHub-Tab waehlen → Felder:

| Feld | Wert |
|---|---|
| PyPI Projektname | `zerodds` |
| Eigentuemer | `zero-objects` |
| Repository-Name | `zero-dds` |
| Workflow name | `publish-pypi.yml` |
| Environment name | `pypi` |

3. "Hinzufuegen" druecken.
4. Analog auf https://test.pypi.org/manage/account/publishing/ — selbe
   Werte, aber Environment name: `testpypi`.

### 1.3 GitHub-Environments anlegen
Repo → Settings → Environments → "New environment":
* Name: `pypi`
* Name: `testpypi`

Die Environments koennen leer bleiben — ihre Existenz allein gated
den OIDC-Token-Exchange. Optional: "Required reviewers" eintragen
fuer Approval-Wall vor dem Push.

### 1.4 Keine GitHub-Secrets noetig
Trusted Publisher tauscht das GitHub-OIDC-Token automatisch gegen einen
kurzlebigen Upload-Token — kein `PYPI_API_TOKEN` zu setzen.

### 1.5 Pipeline triggern
1. **Test-Push zuerst:**
   Actions → publish-pypi → Run workflow → `target_index=testpypi`
   → "Run workflow"
2. Nach Erfolg pruefen: https://test.pypi.org/project/zerodds/
3. Live-installieren testen:
   ```bash
   pip install --index-url https://test.pypi.org/simple/ \
               --extra-index-url https://pypi.org/simple/ \
               zerodds
   ```
4. **Production-Push:**
   Actions → publish-pypi → Run workflow → `target_index=pypi`

### 1.6 Doku-Update nach Live
* `website/bindings/python/index.html`: Schritt 1 Disclaimer raus,
  `pip install zerodds` als Default zeigen.
* `docs/packaging/language-binding-publish-followup.md`: PyPI-Block
  auf "Status: ✅ done" setzen.

---

## 2 · NuGet (`dotnet add package ZeroDDS`)

**Pipeline:** `.github/workflows/publish-nuget.yml`
**Aufwand:** 10 min Setup, sofort live.

### 2.1 Account
https://www.nuget.org/users/account/LogOn — Login via Microsoft- oder
GitHub-Account.

### 2.2 API-Key generieren
1. nuget.org → "Manage API Keys" → "Create"
2. Key Name: `zerodds-ci`
3. Expiration: 365 days
4. Package Owner: dein Account-Name
5. Scopes: ✅ "Push new packages and package versions"
6. Glob Pattern: `ZeroDDS*`
7. Key kopieren — wird nur einmal angezeigt.

### 2.3 GitHub-Secret
| Name | Wert |
|---|---|
| `NUGET_API_KEY` | `oy2...` (von nuget.org) |

### 2.4 Pipeline triggern
1. **Dry-run zuerst:**
   Actions → publish-nuget → Run workflow → ✅ `dry_run`
   → "Run workflow"
   Pruefen ob die nupkg-Artefakte sauber aussehen.
2. **Echter Push:**
   Actions → publish-nuget → Run workflow → ☐ `dry_run`
3. Verifikation: https://www.nuget.org/packages/ZeroDDS (kann
   bis zu ein paar Minuten Index-Lag haben)

### 2.5 Doku-Update nach Live
* `website/bindings/csharp/index.html`: Schritt 1 Disclaimer raus.
* Followup-Plan auf "NuGet: ✅ done".

---

## 3 · npm (`npm install @zerodds/node`)

**Pipeline:** `.github/workflows/publish-npm.yml`
**Aufwand:** 15 min Setup (Org-Reservierung mit eingeschlossen).

### 3.1 Account + Org
1. npm-Account: https://www.npmjs.com/signup
2. Org reservieren: Account → "Add Organization" → name=`zerodds`
   (kostenlos solange nur public packages). Damit ist `@zerodds/*`
   gesperrt.

### 3.2 Token generieren
1. npmjs.com → "Access Tokens" → "Generate New Token" → "Granular Access Token"
2. Token name: `zerodds-ci`
3. Expiration: 365 days
4. Packages and scopes:
   - Selected packages and scopes
   - Add `@zerodds/*`
   - Permission: Read and Write
5. Token kopieren (`npm_...`) — wird nur einmal angezeigt.

### 3.3 GitHub-Secret
| Name | Wert |
|---|---|
| `NPM_AUTH_TOKEN` | `npm_...` (von npmjs.com) |

### 3.4 Pipeline triggern
1. **Dry-run zuerst:**
   Actions → publish-npm → Run workflow → tag=`rc`, ✅ `dry_run`
   Pruefen ob das tarball sauber aussieht.
2. **Echter Push:**
   Actions → publish-npm → Run workflow → tag=`rc`, ☐ `dry_run`
3. Verifikation: https://www.npmjs.com/package/@zerodds/node

Anmerkung: dist-tag `rc` zeigt auf `npm install @zerodds/node@rc`.
Erst nach `1.0.0` final auf `latest` umschalten:
```bash
npm dist-tag add @zerodds/node@1.0.0 latest
```

### 3.5 Doku-Update nach Live
* `website/bindings/typescript-node/index.html`: Schritt 1 Disclaimer
  raus, klar machen dass libzerodds als System-Dep noch separat
  installiert werden muss (koffi nutzt FFI gegen die SO).
* Followup-Plan auf "npm: ✅ done".

---

## 4 · Maven Central (`org.zerodds:omgdds`)

**Pipeline:** `.github/workflows/publish-maven.yml`
**Aufwand:** 1-3 Werktage wegen Sonatype-Namespace-Approval + GPG-Setup.

### 4.1 Sonatype-Central-Portal
1. Account: https://central.sonatype.com/account
2. Namespace beantragen: "Register new namespace"
   - Namespace: `org.zerodds`
   - Verifikation: TXT-Record im DNS:
     ```
     OSSRH-NNNNN.zerodds.org. TXT "<verification-code>"
     ```
3. Approval-Wartezeit: 1-3 Werktage (manuelle Sonatype-Review).

### 4.2 User-Token im Portal
Nach Approval:
1. central.sonatype.com → "View Account" → "Generate User Token"
2. Liefert `<username>` + `<token>` Paar.

### 4.3 GPG-Key fuer Signierung
Maven Central verlangt PGP-Signaturen auf allen Artefakten.

```bash
# Key generieren (RSA 4096, kein Expiration, mail-only)
gpg --full-generate-key
# Long key-id finden
gpg --list-secret-keys --keyid-format LONG
# Privater Key exportieren (fuer GitHub Secret)
gpg --armor --export-secret-keys <KEYID> > /tmp/maven-gpg.asc
# Public Key auf Keyserver pushen (Maven Central pollt diese)
gpg --keyserver hkps://keys.openpgp.org --send-keys <KEYID>
gpg --keyserver hkps://keyserver.ubuntu.com --send-keys <KEYID>
```

### 4.4 pom.xml ergaenzen (vor erstem Push)
`crates/java-omgdds/java/pom.xml` braucht zusaetzlich:
* `<name>`, `<description>`, `<url>` (sind teilweise da)
* `<licenses>` (Apache-2.0 + MIT)
* `<scm>` (Git-URL)
* `<developers>` (Contact)
* `<distributionManagement>` mit `<repository id="central">`
* Plugins: `maven-source-plugin`, `maven-javadoc-plugin`,
  `maven-gpg-plugin`, `central-publishing-maven-plugin`

Vollstaendige Vorlage in
[`language-binding-publish-followup.md`](language-binding-publish-followup.md)
Sektion "Sprint 3 (Maven Central)".

Die Pipeline `publish-maven.yml` faengt fehlende Pflicht-Elemente ab und
faellt mit klarer Meldung — ein erster `dry_run`-Lauf zeigt was fehlt.

### 4.5 GitHub-Secrets
| Name | Wert |
|---|---|
| `SONATYPE_USERNAME` | Portal-User-Token-Username |
| `SONATYPE_PASSWORD` | Portal-User-Token-Wert |
| `MAVEN_GPG_PRIVATE_KEY` | Inhalt von `/tmp/maven-gpg.asc` |
| `MAVEN_GPG_PASSPHRASE` | GPG-Key-Passwort |

### 4.6 Pipeline triggern
1. **Dry-run zuerst** (kein deploy, nur build + sign):
   Actions → publish-maven → Run workflow → ✅ `dry_run`
2. **Staged-Deploy** (geht in Sonatype-Staging-Repo, dort manuell
   "Release" druecken im Portal):
   Actions → publish-maven → Run workflow → ☐ `dry_run`
3. central.sonatype.com → "Publishing" → Staged Repository sehen, dann
   "Publish" druecken.
4. Maven Central Sync: 10-30 min bis das Artefakt auf
   https://repo1.maven.org/maven2/org/zerodds/omgdds/ erscheint.

### 4.7 Doku-Update nach Live
* `website/bindings/java/index.html`: Source-Build-Disclaimer raus,
  Maven-Dep direkt zeigen.
* Followup-Plan auf "Maven: ✅ done".

---

## Cross-Refs

* Pipeline-Sources: `.github/workflows/publish-{pypi,npm,maven,nuget}.yml`
* RC3-Plan: [`language-binding-publish-followup.md`](language-binding-publish-followup.md)
* OPEN-ITEMS-Index: [`../OPEN-ITEMS.md`](../OPEN-ITEMS.md) → "Packaging / Distribution"
* Existierende Vorbilder: `publish-deb.yml`, `publish-rpm.yml`, `release.yml`

## Reihenfolge — empfohlener Tag

Eine moegliche Reihenfolge fuer einen einzigen Setup-Tag:

| Block | Channel | Dauer | Bemerkung |
|---|---|---|---|
| 09:00 | Maven Sonatype-Namespace beantragen (DNS TXT) | 15 min | laeuft im Hintergrund, Approval kommt spaeter |
| 09:15 | GPG-Key generieren | 20 min | Bandbreite-bedingt fuer Entropie |
| 09:35 | PyPI Account + Token + Secret | 15 min | Erst-Push lokal verifizieren |
| 09:50 | PyPI Test + Production Run | 15 min | erste live-Welle ✅ |
| 10:05 | NuGet Account + Token + Secret + Push | 20 min | zweite live-Welle ✅ |
| 10:25 | npm Account + Org + Token + Secret + Push | 25 min | dritte live-Welle ✅ |
| (1-3 Tage spaeter) | Maven Approval-Mail | — | dann pom.xml ergaenzen + Run |

In ~80 min sind 3 von 4 Channels live; Maven kommt nach Sonatype-Approval.
