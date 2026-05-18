# Track 10-C — Marketing-Launch (zeitkoordiniert)

**Goal:** koordinierte 1.0-Ankündigung an die richtige Audience zum
richtigen Zeitpunkt.

**Status:** 📋 todo (gated auf OMG-Vendor-ID + News-Sektion + 1.0-Tag)

## Audience-Hierarchie

User-Klarstellung: **kein Blog**, **News-Sektion erst nach Vendor-ID**.
Die Marketing-Strategie ist dementsprechend nüchtern: **die richtigen
Leute auf der richtigen Liste informieren**, kein viraler Content-Push.

| Tier | Audience | Channel | Timing |
|---|---|---|---|
| 1 | OMG-TC-Liste (DDS-Working-Group) | Mail-Liste | Tag 0 (vor allen anderen) |
| 1 | Eclipse-Cyclone-Mailingliste | Mail-Liste | Tag 0 |
| 2 | Embedded.com / DDS-relevante Trade-Publications | Pressemitteilung | Tag 1 |
| 2 | Hacker News (Show HN: ZeroDDS 1.0) | submission | Tag 1 |
| 3 | RustConf / FOSDEM / embedded world Submissions | CFP | sobald CFPs offen |
| 3 | Industry-Mailing-Listen (CIP, Linux-Foundation Edge) | targeted notes | Tag 2-7 |

**Bewusst NICHT bedient:**
- Twitter/X / Mastodon — niedriger ROI für Industrial-Audience
- LinkedIn-Posts — irrelevant für die wirklichen Entscheider
- YouTube-Tutorials — separater Track post-1.0 wenn überhaupt
- Reddit /r/programming oder /r/rust — viraler aber wrong audience

## Pre-Launch-Checklist

- [ ] OMG-Vendor-ID erteilt (10-A done)
- [ ] News-Sektion live (10-B done)
- [ ] 1.0-final cargo-publish durch
- [ ] Distribution-Channels (apt/dnf/brew/scoop/AUR/Gentoo/Docker) zeigen
      1.0.0-Tag, nicht mehr -rc.X
- [ ] Whitepaper "Cross-Vendor DDS Performance" als PDF auf der News-
      Sektion
- [ ] Spec-Coverage-Page aktualisiert: alle Vendor-IDs auf zugeteilte ID
      umgestellt
- [ ] Documentation Trail finalisiert mit 1.0-Sample-Output

## Pressemitteilung

Template-Inhalt (~300 Wörter, sachlich):

```
ZeroDDS 1.0 released — production-grade pure-Rust OMG DDS
implementation with full cross-vendor wire-compatibility

[Datum, Ort] ZeroDDS 1.0 has been released under Apache License 2.0,
implementing the full OMG Data Distribution Service stack: DCPS 1.4,
DDSI-RTPS 2.5, XTypes 1.3, DDS-Security 1.2, plus seven protocol
bridges (WebSocket, MQTT, CoAP, AMQP 0.9 + 1.0, gRPC, CORBA, ROS-2)
and language bindings for ten runtimes.

Performance audit shows ZeroDDS roundtrip latency 30-150 % below
Cyclone DDS, RTI Connext and eProsima Fast DDS on equivalent
benchmark hardware.

The release is available via apt, dnf, brew, scoop, AUR, Gentoo
overlay and ghcr.io Docker images. OMG Vendor-ID assigned by the
Object Management Group.

[Links zu spec-coverage, whitepaper, github]

Apache 2.0 License — github.com/zero-objects/zero-dds
zerodds.org
```

## Conference-Submissions (post-launch)

CFPs zu beobachten:
- **OMG TC-Meeting** (quartalsweise) — Vendor-Implementations-Showcase
- **RustConf 2026** (CFP typischerweise April-Juni) — Talk "pure-Rust
  DDS without the cargo bloat"
- **FOSDEM 2027** (CFP typischerweise Oktober-November) — Embedded /
  Real-Time Devroom
- **embedded world 2027** — Industrial-IoT-Track
- **EclipseCon 2026** — wenn Eclipse-Cyclone-Track relevant

Pro Submission: ein abstract, eine slide-deck, ein 2-min-demo-video.

## Acceptance

1. Pressemitteilung an OMG-TC-Liste verschickt + ack
2. HN-Submission live (auch wenn nicht front-page — die Submission selbst
   ist der reach)
3. embedded.com-Anfrage gestellt
4. mind. 1 Conference-Submission (RustConf oder FOSDEM) eingereicht
5. Web-analytics zeigen: nicht-zero traffic auf zerodds.org in den 7
   Tagen post-launch

## Out-of-Scope

- Paid-advertising
- Influencer-Outreach
- Social-Media-Manager-Rolle
- Newsletter-Subscription-System

## Dependencies

- 10-A + 10-B abgeschlossen
- Whitepaper-Draft fertig
