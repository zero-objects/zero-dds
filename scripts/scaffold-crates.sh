#!/usr/bin/env bash
# Generiert Skelette fuer alle Workspace-Crates.
# Einmalig auszufuehren; idempotent (ueberschreibt keine existierenden lib.rs).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Format: name|path|kind|safety|no_std|description
# kind:    lib | bin
# safety:  safe | standard | comfort | tooling
#          - safe/standard/comfort: Runtime-Crates gemaess 02_architecture.md §4.4.1
#          - tooling: Meta-Tools (kein Runtime-Code, nicht safety-klassifiziert),
#            default = [] statt std/alloc/safety
# no_std:  yes | no | optional
CRATES=(
  # Foundation
  "zerodds-foundation|crates/foundation|lib|safe|yes|Core types: GUID, SequenceNumber, Time, Duration, Error families"

  # Protocol Layer
  "zerodds-cdr|crates/cdr|lib|safe|yes|XCDR1/XCDR2 encoder/decoder, endianness, alignment"
  "zerodds-types|crates/types|lib|safe|yes|XTypes type system, TypeObject, TypeIdentifier, compatibility"
  "zerodds-qos|crates/qos|lib|safe|yes|QoS policies, request/offered compatibility matrix"
  "zerodds-idl|crates/idl|lib|safe|no|IDL4 parser, AST, and semantic model (OMG IDL 4.2 / ISO/IEC 19516) — std-only by design, siehe RFC 0001"
  "zerodds-rtps|crates/rtps|lib|safe|yes|Writer/Reader state machines, RTPS submessages, fragmentation"
  "zerodds-discovery|crates/discovery|lib|safe|yes|SPDP, SEDP, TypeLookup service"

  # Transport Layer
  "zerodds-transport|crates/transport|lib|safe|yes|Transport trait, Locator, abstract send/receive"
  "zerodds-transport-udp|crates/transport-udp|lib|safe|optional|UDP/IP PSM, raw socket, multicast"
  "zerodds-transport-tcp|crates/transport-tcp|lib|standard|no|DDSI TCP/IP PSM, connection pool"
  "zerodds-transport-shm|crates/transport-shm|lib|safe|optional|Shared-memory segments, zero-copy path"

  # Core Services
  "zerodds-dcps|crates/dcps|lib|standard|no|DomainParticipant, Publisher, Subscriber, Topic, DataReader, DataWriter"
  "zerodds-rpc|crates/rpc|lib|standard|no|DDS-RPC request/reply framework"
  "zerodds-security|crates/security|lib|safe|optional|Authentication/AccessControl/Cryptographic plugin SPI"
  "zerodds-xml|crates/xml|lib|standard|no|DDS-XML parser, QoS-profile loader, schema validator"
  "zerodds-xrce-client|crates/xrce-client|lib|safe|yes|XRCE client for Micro profile, transport-agnostic"
  "zerodds-xrce-agent|crates/xrce-agent|lib|standard|no|XRCE agent running in Full/Standard profile"
  "zerodds-recorder|crates/recorder|lib|comfort|no|Deterministic record/replay service"
  "zerodds-monitor|crates/monitor|lib|comfort|no|OpenTelemetry instrumentation, Prometheus exporter, wire probe"

  # Bindings
  "zerodds-rs|crates/rs|lib|standard|no|Idiomatic Rust SDK, async/await, streams"
  "zerodds-sys|crates/sys|lib|safe|optional|Stable C-ABI, basis for non-Rust bindings"
  "zerodds-cpp|crates/cpp|lib|standard|no|C++ wrapper and IDL4-C++ runtime"
  "zerodds-cs|crates/cs|lib|standard|no|C# P/Invoke, NativeAOT-compatible, IDL4-C# runtime"
  "zerodds-java|crates/java|lib|standard|no|JNI bindings, IDL4-Java runtime"
  "zerodds-py|crates/py|lib|comfort|no|PyO3 bindings, pandas/numpy-friendly"

  # Meta-Tooling (Library-Crates, kein Runtime)
  "zerodds-lint|crates/lint|lib|tooling|no|Custom Clippy-Lints und Projekt-Regeln fuer ZeroDDS (dds_no_panic_in_safe, dds_require_safety_comment, dds_spec_annotated, ...)"

  # Binary Tools
  "zerodds-idlc|tools/idlc|bin|comfort|no|IDL4 compiler: backends for C, C++, C#, Java, Python, Rust"
  "zerodds-admin|tools/admin|bin|comfort|no|Admin CLI: domain inspector, QoS validator, discovery snapshot"
  "zerodds-xmlc|tools/xmlc|bin|comfort|no|DDS-XML validator, schema checker, deployment renderer"
  "zerodds-dashboard|tools/dashboard|bin|comfort|no|Tauri app for live monitoring, discovery graph, replay browser"
  "zerodds-perf|tools/perf|bin|comfort|no|Load generator, latency profiler, benchmark suite"
  "zerodds-traceability|tools/traceability|bin|comfort|no|Requirements-to-code matrix generator"
)

write_cargo_toml() {
  local name="$1" path="$2" kind="$3" desc="$4" safety="${5:-}"
  cat > "$path/Cargo.toml" <<EOF
[package]
name = "$name"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
description = "$desc"
publish = false

[lints]
workspace = true

EOF
  if [ "$kind" = "lib" ]; then
    if [ "$safety" = "tooling" ]; then
      # Meta-Tooling: keine Profile-Features.
      cat >> "$path/Cargo.toml" <<EOF
[lib]
path = "src/lib.rs"

[features]
default = []
EOF
    else
      cat >> "$path/Cargo.toml" <<EOF
[lib]
path = "src/lib.rs"

[features]
default = ["std"]
std = ["alloc"]
alloc = []
safety = []
EOF
    fi
  else
    cat >> "$path/Cargo.toml" <<EOF
[[bin]]
name = "$name"
path = "src/main.rs"
EOF
  fi

  cat >> "$path/Cargo.toml" <<EOF

[dependencies]

[dev-dependencies]
EOF
}

write_lib_rs() {
  local name="$1" path="$2" safety="$3" no_std="$4" desc="$5"
  local file="$path/src/lib.rs"
  [ -f "$file" ] && return 0

  # Header gemaess 04_safety_by_architecture.md §2
  {
    echo "//! $desc"
    echo "//!"
    echo "//! Crate \`$name\`."
    echo "//!"
    echo "//! Safety classification: **$(echo "$safety" | tr '[:lower:]' '[:upper:]')**."
    echo "//! Siehe \`docs/architecture/02_architecture.md §3\` und"
    echo "//! \`docs/architecture/04_safety_by_architecture.md §2\`."
    echo ""
    # no_std Gate
    if [ "$no_std" = "yes" ]; then
      echo "#![no_std]"
    elif [ "$no_std" = "optional" ]; then
      echo "#![cfg_attr(not(feature = \"std\"), no_std)]"
    fi
    # Unsafe-Politik gemaess 02_architecture.md §4.4.1
    case "$safety" in
      safe|tooling)
        echo "#![forbid(unsafe_code)]"
        ;;
      standard)
        echo "#![deny(unsafe_code)]"
        ;;
      comfort)
        echo "#![warn(unsafe_code)]"
        ;;
    esac
    echo "#![warn(missing_docs)]"
    echo ""
    # alloc-Brueckenkopf fuer no_std+alloc
    if [ "$no_std" = "yes" ] || [ "$no_std" = "optional" ]; then
      echo "#[cfg(feature = \"alloc\")]"
      echo "extern crate alloc;"
      echo ""
    fi
    echo "// TODO: Implementierung folgt. Platzhalter fuer Phase-0-CI-Verdrahtung."
    echo ""
    echo "#[cfg(test)]"
    echo "mod tests {"
    echo "    #[test]"
    echo "    fn crate_compiles() {"
    echo "        // Smoke-Test: Crate kompiliert und Testharness laeuft."
    echo "    }"
    echo "}"
  } > "$file"
}

write_main_rs() {
  local name="$1" path="$2" desc="$3"
  local file="$path/src/main.rs"
  [ -f "$file" ] && return 0
  {
    echo "//! $desc"
    echo "//!"
    echo "//! Binary \`$name\`."
    echo ""
    echo "#![allow(clippy::print_stderr)] // CLI-Tool: stderr fuer Fehler zulaessig (Policy: tools/* duerfen lockern)."
    echo ""
    echo "fn main() {"
    echo "    // TODO: Implementierung folgt."
    echo "    eprintln!(\"$name: not yet implemented\");"
    echo "    std::process::exit(2);"
    echo "}"
  } > "$file"
}

for entry in "${CRATES[@]}"; do
  IFS='|' read -r name path kind safety no_std desc <<< "$entry"
  write_cargo_toml "$name" "$path" "$kind" "$desc" "$safety"
  if [ "$kind" = "lib" ]; then
    write_lib_rs "$name" "$path" "$safety" "$no_std" "$desc"
  else
    write_main_rs "$name" "$path" "$desc"
  fi
  echo "  scaffolded: $name ($path, $safety, no_std=$no_std)"
done

echo ""
echo "Fertig. ${#CRATES[@]} Crates/Tools."
