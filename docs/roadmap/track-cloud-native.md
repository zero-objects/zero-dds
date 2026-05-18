# Track Post-A — Cloud-Native (Detail)

**Status:** 📋 backlog (post-1.0)

**Trigger:** Industry-Demand für K8s-deployments. Vor erstem Customer-
Request ist die Investition spekulativ.

## Items (priorisierte Reihenfolge)

1. **Helm-Chart** (1 PW) — `charts/zerodds/` mit deployment für die 7
   Bridge-Daemons + Persistence-Service. Trigger-Item (kleine investment,
   großer Effekt für Kubernetes-User).
2. **OpenTelemetry-Dashboards** (0.5 PW) — `packaging/grafana/*.json` mit
   den Standard-Histogrammen aus `zerodds-observability-otlp`.
3. **Kubernetes Operator** (3-4 PW) — CRDs für Participant, Topic,
   Bridge. Operator-SDK-Pattern (Rust via `kube-rs`).
4. **Knative Eventing Source/Sink** (1 PW) — DDS-Topic als
   CloudEvents-Source und -Sink, für Serverless-Pipelines.
5. **Service-Mesh-Integration** (1 PW) — Istio/Linkerd-Annotations,
   mTLS-Inheritance, Trace-Propagation über bridge-security.

## Acceptance pro Item

Standard: lauffähiger Helm-Install / Operator-Reconcile / OpenTelemetry-
Dashboard mit Live-Metrics, jeweils dokumentiert.

## Out-of-Scope

- Eigener Cloud-Provider — wir bleiben on-prem-friendly
- DDS-as-a-Service Hosted-Offering — nicht unser Geschäftsmodell

## Dependencies

- 1.0-final published (Helm-Chart referenziert Image-Tags)
- ghcr.io stabile Image-Tags
