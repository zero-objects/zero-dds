# User Guide

For application developers building systems **on top of** ZeroDDS.

## Planned Sections

- **Getting Started** — install, first publisher, first subscriber, one per
  supported language binding (Rust, C, C++, C#, Java, Python).
- **Core Concepts** — domains, topics, QoS, discovery, type system.
  Reference-style, cross-linked to the OMG specs.
- **Profiles** — when to pick Full / Standard / Safe / Micro. See
  `docs/architecture/03_profiles_and_platforms.md §8` for the decision flow.
- **IDL Workflow** — writing IDL, generating bindings with `zerodds-idlc`,
  evolving types.
- **QoS Cookbook** — recipes for common scenarios (reliable broadcast,
  durable state, partitioned workloads).
- **Security** — enabling DDS-Security, configuring PKI, writing
  permissions documents.
- **Troubleshooting** — common interop issues, discovery debugging,
  reading logs.

## Status

This directory is a legacy breadcrumb. The current user-oriented
content lives in [`../01-getting-started/`](../01-getting-started/)
and [`../05-integration/`](../05-integration/).
