# Phase 1.0-final

**Goal:** öffentlicher 1.0-Release-Candidate-Cleanup, OMG-Vendor-ID-
Vergabe abwarten, koordinierter Launch mit News-Sektion-Befüllung.

**Status:** 📋 todo (gated auf RC3 abgeschlossen + OMG-Vendor-ID erteilt)

## Tracks

| # | Track | Detail-Doku |
|---|---|---|
| 10-A | OMG-Vendor-ID-Vergabe | [`track-omg-vendor-id.md`](track-omg-vendor-id.md) |
| 10-B | News-Sektion auf der Website befüllen | [`track-news-section-launch.md`](track-news-section-launch.md) |
| 10-C | Marketing-Launch (zeitkoordiniert) | [`track-marketing-launch.md`](track-marketing-launch.md) |

## Wann startet Phase 10

- RC3 alle Acceptance-Criteria grün
- OMG-Vendor-ID erteilt (oder zumindest in-progress mit klaren ETA)
- Alle Demos + Tutorials audited (RC2-C, D)
- Cargo-publish in DAG-Order auf rc.3 fehlerfrei

## Phase-Acceptance

- Workspace-Tag `1.0.0-final` ohne `-rc`-Suffix
- OMG-Vendor-ID im RTPS-ProtocolHeader vendor_id-Feld
- Vendor-ID-pending-Banner auf der Website entfernt
- News-Sektion zerodds.org/news/ live mit:
  - 1.0-Release-Announcement
  - OMG-Vendor-ID-Acknowledgement
  - Performance-Whitepaper als verlinktes PDF
- Pressemitteilung an OMG-TC-Liste + Embedded.com + Hacker News (in
  dieser Reihenfolge — OMG-Liste zuerst, ist die richtige Audience)

## Was NICHT zu Phase 10 gehört

- Eigener Blog (User: "albern")
- Discord/Slack-Community-Server
- Conference-Submissions (separater post-1.0 Track)
- Industry-Vertical-Anbahnung (separater post-1.0 Track)
