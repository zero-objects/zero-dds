# Operator Guide

For platform, DevOps, and SRE engineers **deploying and running** systems
built on ZeroDDS.

## Planned Sections

- **Deployment Models** — single-host, multi-host, containerized,
  Kubernetes with sidecar, RTOS integration.
- **Network Planning** — multicast requirements, NAT/firewall scenarios
  with the TCP PSM, locator configuration.
- **Observability Setup** — wiring up the OpenTelemetry stack, Prometheus
  scraping, Grafana dashboards, alert rules. Canonical source:
  `docs/architecture/05_observability_and_tooling.md`.
- **Security Operations** — PKI management, certificate rotation, HSM
  integration, incident response.
- **Recording & Replay** — capturing wire traffic, storage, replay for
  post-mortem analysis.
- **Capacity Planning** — sizing guidance, QoS tuning, resource
  requirements per profile.
- **Troubleshooting Playbooks** — discovery failures, high retransmit
  rates, deadline misses, security handshake problems.

## Status

This directory is a legacy breadcrumb. The current operator-
oriented content lives in [`../03-configuration/`](../03-configuration/)
and [`../06-operations/`](../06-operations/).
