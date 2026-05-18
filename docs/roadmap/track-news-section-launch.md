# Track 10-B — News-Sektion auf der Website

**Goal:** News-Sektion auf zerodds.org/news/ launched mit dem
1.0-Release-Announcement, OMG-Vendor-ID-Acknowledgement, und Whitepaper-
Links. **Vor 1.0-final + OMG-ID nicht aktivieren.**

**Status:** 📋 todo (gated auf OMG-Vendor-ID erteilt)

## Designentscheidung

User-Klarstellung: **kein eigener Blog**, sondern eine News-Sektion. Der
Unterschied:

| Blog | News-Sektion |
|---|---|
| persönlich, Engineering-Geschichten, Meinungs-Posts | sachlich, jedes Item ist ein verifizierbares Event |
| regelmäßiger Cadence (wöchentlich/monatlich) erwartet | sporadisch, nur wenn echtes News |
| viele Read-Posts | wenige aber substantielle Posts |
| Blog-Post-Schreiber als "Autor" | News-Items sind Projekt-Events ohne persönliche Autorschaft |

**Was als News qualifiziert:**
- Spec-Releases (1.0, 1.1, 2.0)
- OMG-Vendor-ID-Acknowledgement
- Major-Cross-Vendor-Compliance-Verifikation (z.B. mit RTI in der
  vendor-Conformance-DB)
- Industry-Adoption (mit Logo-Permission)
- Performance-Records (z.B. erstes pure-Rust-DDS unter 1 µs roundtrip)
- Security-Advisories (CVE-Listings)
- Conformance-Audit-Reports von externen Auditoren

**Was NICHT als News qualifiziert:**
- Sprint-Recap, Engineering-Anekdoten
- Personal-News (wer ist Maintainer)
- Future-Plans (das gehört in roadmap/, nicht news/)
- Drittparteien-Tutorials oder Talks (die linken wir aus, nicht News)

## Implementierung

### Verzeichnis

- `website/news/index.html` — chronologische Liste aller News-Items
  (newest first), Article-Layout
- `website/news/<YYYY-MM-DD>-<slug>.html` — pro Item, Article-Layout
  mit Eyebrow + Lede + Datum + Body + Permalink

### Beispiel-Items für 1.0-Launch

```
2026-XX-XX  ZeroDDS 1.0.0 — Production-Grade Release
2026-XX-XX  OMG Vendor-ID 0xXXXX assigned
2026-XX-XX  Cross-Vendor Performance Audit: ZeroDDS Tier-A vs. Cyclone, RTI, Fast-DDS
```

### Atom/RSS-Feed

`website/news/atom.xml` — RFC-4287 Atom-Feed, generated von einem
build-script (in `_tools/render_news.py`).

### Sidebar in main-Nav

Im `assets/layout.js` ergänzen:

```js
${navItem("news", "/news/index.html", "news")}
```

Plus translations key `nav.news` = "News" (en) / "Neuigkeiten" (de).

### Erst aktivieren

Bis OMG-ID + 1.0-Tag: News-Sektion **NICHT** im Top-Nav verlinkt.
`website/news/` existiert aber leer (keine `index.html`), die Nav-Item
wird vor dem Launch ergänzt.

## Acceptance

1. zerodds.org/news/ ist erreichbar (200) ab 1.0-final-Tag
2. mind. 3 News-Items zum Launch live (1.0-Release, OMG-ID, Whitepaper)
3. Atom-Feed valide gegen W3C-Atom-Validator
4. Sidebar-Nav-Item live in DE + EN
5. Linkbarkeit: HN-Submission, OMG-TC-Mailingliste, embedded.com — alle
  Links auf zerodds.org/news/ funktionieren

## Dependencies

- 10-A OMG-Vendor-ID erteilt
- 1.0-final-Tag aus dem Workspace
- Whitepaper-Draft (entstand intern in RC2-A)
