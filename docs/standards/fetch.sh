#!/usr/bin/env bash
# docs/standards/fetch.sh
#
# Laedt die frei verfuegbaren Specs in den lokalen Cache `cache/`.
# Idempotent — ueberspringt bereits vorhandene Dateien (es sei denn --force).
# Paywalled Specs (ISO/IEC, DO-178C, EN 50128) werden nur mit Bezugs-Hinweis
# gemeldet, nicht heruntergeladen.
#
# Aufruf:
#     ./docs/standards/fetch.sh              # normaler Lauf
#     ./docs/standards/fetch.sh --force      # erzwingt Neu-Download aller Dateien
#     ./docs/standards/fetch.sh --dry-run    # zeigt nur was geschehen wuerde
#     ./docs/standards/fetch.sh --only=omg   # laedt nur eine Kategorie (omg|w3c|ietf|otel|misc)
#
# Die URLs spiegeln den Stand von Draft v0.2 der Architektur-Docs wider.
# Bei Aenderungen: Eintraege in INDEX.md, omg.md bzw. non-omg.md zuerst
# aktualisieren, dann dieses Script.

set -euo pipefail

# --- Konfiguration ----------------------------------------------------------

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
CACHE_DIR="$SCRIPT_DIR/cache"

FORCE=0
DRY_RUN=0
ONLY=""

for arg in "$@"; do
    case "$arg" in
        --force)     FORCE=1 ;;
        --dry-run)   DRY_RUN=1 ;;
        --only=*)    ONLY="${arg#--only=}" ;;
        -h|--help)
            sed -n '2,22p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

# --- Hilfs-Funktionen -------------------------------------------------------

log()  { printf '  %s\n' "$*"; }
info() { printf '\e[1;34m==> %s\e[0m\n' "$*"; }
warn() { printf '\e[1;33m!!  %s\e[0m\n' "$*"; }
skip() { printf '    \e[2m[skip]\e[0m %s\n' "$*"; }

want_category() {
    [[ -z "$ONLY" || "$ONLY" == "$1" ]]
}

# fetch <category> <url> <relative-path-under-cache>
fetch() {
    local category="$1" url="$2" rel="$3"
    want_category "$category" || return 0

    local dest="$CACHE_DIR/$rel"
    local dir
    dir="$(dirname -- "$dest")"

    if [[ $FORCE -eq 0 && -s "$dest" ]]; then
        skip "$rel"
        return 0
    fi

    if [[ $DRY_RUN -eq 1 ]]; then
        log "would fetch: $url -> $rel"
        return 0
    fi

    mkdir -p "$dir"
    log "fetching $rel"
    if ! curl --location --silent --show-error --fail --output "$dest" "$url"; then
        warn "failed: $url"
        rm -f "$dest"
    fi
}

# note_paywalled <label> <purchase-url>
note_paywalled() {
    want_category "iso" || return 0
    warn "paywalled — $1"
    log "   acquire via: $2"
}

# --- OMG --------------------------------------------------------------------

info "OMG Data Distribution Service specifications"
# OMG PDF-URLs folgen dem Muster https://www.omg.org/spec/<SPEC>/<VERSION>/PDF
fetch omg "https://www.omg.org/spec/DDS/1.4/PDF"              "omg/zerodds-dcps-1.4.pdf"
fetch omg "https://www.omg.org/spec/DDSI-RTPS/2.5/PDF"        "omg/ddsi-rtps-2.5.pdf"
fetch omg "https://www.omg.org/spec/DDS-XTypes/1.3/PDF"       "omg/dds-xtypes-1.3.pdf"
fetch omg "https://www.omg.org/spec/DDS-SECURITY/1.2/PDF"     "omg/zerodds-security-1.2.pdf"
fetch omg "https://www.omg.org/spec/IDL/4.2/PDF"              "omg/idl-4.2.pdf"
fetch omg "https://www.omg.org/spec/DDS-RPC/1.0/PDF"          "omg/zerodds-rpc-1.0.pdf"
fetch omg "https://www.omg.org/spec/DDS-XML/1.0/PDF"          "omg/zerodds-xml-1.0.pdf"
fetch omg "https://www.omg.org/spec/DDS-XRCE/1.0/PDF"         "omg/zerodds-xrce-1.0.pdf"

# Sekundaer-Kunden Spec-Bedarf — siehe TODO_open_implementations.md.
# DDS-WEB (Web-Enabled-DDS): RESTful + WebSocket Gateway zu DDS.
fetch omg "https://www.omg.org/spec/DDS-WEB/1.0/PDF"          "omg/zerodds-web-1.0.pdf"
# DDS-TSN (1.0 Beta2): DDS over Time-Sensitive Networking.
fetch omg "https://www.omg.org/spec/DDS-TSN/1.0/Beta2/PDF"    "omg/dds-tsn-1.0-beta2.pdf"
# DDS4CCM (Lightweight-CCM-Gateway, XML-QoS-Profile-Quelle).
fetch omg "https://www.omg.org/spec/DDS4CCM/1.1/PDF"          "omg/dds4ccm-1.1.pdf"
# DDS-OPCUA: DDS-OPC-UA-Gateway (Industrial-Interop).
fetch omg "https://www.omg.org/spec/DDS-OPCUA/1.0/PDF"        "omg/dds-opcua-1.0.pdf"
# Anmerkungen zu nicht-vorhandenen Specs:
# - "DDS-Cloud": kein OMG-Spec unter diesem Namen (Cloud-Discovery
#   ist Vendor-Feature; OMG-Pendant waere DDS-WEB + Discovery-Service).
# - "DDS-TS"/TypeScript-Binding: kein OMG-Standard (offene RFP);
#   TODO_open_implementations.md dokumentiert das als zukuenftiges
#   Sekundaer-Kunden-Item.
# - TCP-Transport: kein separater OMG-Spec; RTPS-over-TCP ist im
#   DDSI-RTPS Annex (siehe `crates/transport-tcp/`-TODO).

info "OMG DDS-Adjacent Specs"
# TIME 1.1: Time-Service (DDS-Security Clock-Skew, DCPS Time/Duration).
fetch omg "https://www.omg.org/spec/TIME/1.1/PDF"             "omg/time-1.1.pdf"
# AMI4CCM 1.1: Async Method Invocation fuer CCM (DDS-RPC-Async-Quelle).
fetch omg "https://www.omg.org/spec/AMI4CCM/1.1/PDF"          "omg/ami4ccm-1.1.pdf"
# CCM 4.0: CORBA Component Model (Basis hinter DDS4CCM).
fetch omg "https://www.omg.org/spec/CCM/4.0/PDF"              "omg/ccm-4.0.pdf"
# RTC 1.0: Robotic-Technology-Component (DDS-Robotik-Brueckenspec).
fetch omg "https://www.omg.org/spec/RTC/1.0/PDF"              "omg/rtc-1.0.pdf"

info "OMG Language-Mappings"
# Alle OMG-URLs brauchen Versions-Segment (nicht nur /PDF). C#-DDS-PSM existiert
# bei OMG nicht als eigene Spec — nur das IDL4-to-C#-Mapping ist standardisiert.
fetch omg "https://www.omg.org/spec/IDL4-CPP/1.0/PDF"         "omg/idl4-cpp-1.0.pdf"
fetch omg "https://www.omg.org/spec/IDL4-Java/1.0/PDF"        "omg/idl4-java-1.0.pdf"
fetch omg "https://www.omg.org/spec/IDL4-CSHARP/1.0/PDF"      "omg/idl4-csharp-1.0.pdf"
fetch omg "https://www.omg.org/spec/DDS-PSM-Cxx/1.0/PDF"      "omg/dds-psm-cxx-1.0.pdf"
fetch omg "https://www.omg.org/spec/DDS-Java/1.0/PDF"         "omg/zerodds-java-psm-1.0.pdf"

# --- W3C --------------------------------------------------------------------

info "W3C"
fetch w3c "https://www.w3.org/TR/trace-context/"              "w3c/trace-context.html"
fetch w3c "https://www.w3.org/TR/trace-context-2/"            "w3c/trace-context-2.html"

# --- IETF -------------------------------------------------------------------

info "IETF RFCs"
fetch ietf "https://www.rfc-editor.org/rfc/rfc768.txt"        "ietf/rfc768.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc9293.txt"       "ietf/rfc9293.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc1122.txt"       "ietf/rfc1122.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc4122.txt"       "ietf/rfc4122.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc5905.txt"       "ietf/rfc5905.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc8446.txt"       "ietf/rfc8446.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc5280.txt"       "ietf/rfc5280.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc8032.txt"       "ietf/rfc8032.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc5116.txt"       "ietf/rfc5116.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc9110.txt"       "ietf/rfc9110.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc9112.txt"       "ietf/rfc9112.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc9113.txt"       "ietf/rfc9113.txt"
fetch ietf "https://www.rfc-editor.org/rfc/rfc8949.txt"       "ietf/rfc8949.txt"
# CoAP (RFC 7252) — IoT-Pub-Sub, DDS-XRCE-Alternative.
fetch ietf "https://www.rfc-editor.org/rfc/rfc7252.txt"       "ietf/rfc7252.txt"
# CoAP-Observe (RFC 7641) — Pub-Sub-Subscribe-Pendant.
fetch ietf "https://www.rfc-editor.org/rfc/rfc7641.txt"       "ietf/rfc7641.txt"
# WebSocket Protocol (RFC 6455) — Begleitspec zu DDS-WEB.
fetch ietf "https://www.rfc-editor.org/rfc/rfc6455.txt"       "ietf/rfc6455.txt"

# --- CNCF / OpenTelemetry / Prometheus --------------------------------------

info "OpenTelemetry (shallow clone of current main)"
if want_category otel; then
    if [[ $DRY_RUN -eq 1 ]]; then
        log "would shallow-clone opentelemetry-specification and opentelemetry-proto"
    else
        mkdir -p "$CACHE_DIR/otel"
        if [[ ! -d "$CACHE_DIR/otel/specification/.git" || $FORCE -eq 1 ]]; then
            rm -rf "$CACHE_DIR/otel/specification"
            git clone --depth=1 https://github.com/open-telemetry/opentelemetry-specification \
                "$CACHE_DIR/otel/specification" 2>&1 | sed 's/^/    /'
        else
            skip "otel/specification (already cloned; use --force to refresh)"
        fi
        if [[ ! -d "$CACHE_DIR/otel/proto/.git" || $FORCE -eq 1 ]]; then
            rm -rf "$CACHE_DIR/otel/proto"
            git clone --depth=1 https://github.com/open-telemetry/opentelemetry-proto \
                "$CACHE_DIR/otel/proto" 2>&1 | sed 's/^/    /'
        else
            skip "otel/proto (already cloned; use --force to refresh)"
        fi
    fi
fi

info "OpenMetrics"
fetch otel "https://raw.githubusercontent.com/prometheus/OpenMetrics/main/specification/OpenMetrics.md" \
    "openmetrics/openmetrics-1.0.md"

info "Prometheus exposition format"
fetch otel "https://prometheus.io/docs/instrumenting/exposition_formats/" \
    "prometheus/exposition-formats.html"

# --- OASIS ------------------------------------------------------------------

info "OASIS Messaging Standards (DDS-Bridge-Interop)"
# MQTT 5.0 — Pub-Sub-Bridge-Target (IoT-Sekundaerkunden).
fetch misc "https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html" \
    "oasis/mqtt-5.0.html"
# AMQP 1.0 — Enterprise-Messaging-Bridge (Cloud-/Banking-Sekundaerkunden).
fetch misc "https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-overview-v1.0-os.html" \
    "oasis/amqp-1.0-overview.html"
fetch misc "https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-types-v1.0-os.html" \
    "oasis/amqp-1.0-types.html"
fetch misc "https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-transport-v1.0-os.html" \
    "oasis/amqp-1.0-transport.html"
fetch misc "https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-messaging-v1.0-os.html" \
    "oasis/amqp-1.0-messaging.html"
fetch misc "https://docs.oasis-open.org/amqp/core/v1.0/os/amqp-core-security-v1.0-os.html" \
    "oasis/amqp-1.0-security.html"

info "ROS 2 (REP — DDS-RMW-Bridge)"
# REP-2003: Topic and Service Name Mapping to DDS.
fetch misc "https://ros.org/reps/rep-2003.html"               "ros2/rep-2003.html"
# REP-2004: Quality of Service.
fetch misc "https://ros.org/reps/rep-2004.html"               "ros2/rep-2004.html"
# REP-2005: ROS Interfaces.
fetch misc "https://ros.org/reps/rep-2005.html"               "ros2/rep-2005.html"
# REP-2007: Type Description Interface.
fetch misc "https://ros.org/reps/rep-2007.html"               "ros2/rep-2007.html"
# REP-2008: ROS 2 Hardware-Acceleration / DDS-Adapter.
fetch misc "https://ros.org/reps/rep-2008.html"               "ros2/rep-2008.html"
# REP-2009: RMW Middleware Interface.
fetch misc "https://ros.org/reps/rep-2009.html"               "ros2/rep-2009.html"

info "gRPC Core Protocol"
# gRPC over HTTP/2.
fetch misc "https://raw.githubusercontent.com/grpc/grpc/master/doc/PROTOCOL-HTTP2.md" \
    "grpc/protocol-http2.md"
# gRPC-Web (Browser-tauglich).
fetch misc "https://raw.githubusercontent.com/grpc/grpc/master/doc/PROTOCOL-WEB.md" \
    "grpc/protocol-web.md"

info "OASIS PKCS#11"
# v3.1 ist noch nicht als OS-Release publiziert (Stand 2026-04); v3.0 ist der
# aktuelle freigegebene Stand. Bei Veroeffentlichung von v3.1-OS hier umstellen.
fetch misc "https://docs.oasis-open.org/pkcs11/pkcs11-base/v3.0/os/pkcs11-base-v3.0-os.html" \
    "oasis/pkcs11-3.0.html"

# --- Sonstige ---------------------------------------------------------------

info "CycloneDX SBOM"
fetch misc "https://cyclonedx.org/docs/1.6/json/" \
    "cyclonedx/spec-1.6.html"

info "Semantic Versioning"
fetch misc "https://semver.org/spec/v2.0.0.html" \
    "misc/semver-2.0.0.html"

info "Conventional Commits"
fetch misc "https://www.conventionalcommits.org/en/v1.0.0/" \
    "misc/conventional-commits-1.0.0.html"

info "PlatformIO library.json manifest"
fetch misc "https://docs.platformio.org/en/latest/manifests/library-json/index.html" \
    "misc/platformio-library-json.html"

# --- ISO / IEC / DO-178C — paywalled Hinweise -------------------------------

if want_category iso; then
    info "ISO / IEC / DO-178C / EN — paywalled"
    note_paywalled "ISO 26262 (road vehicles functional safety)"    "https://www.iso.org/standard/68383.html"
    note_paywalled "IEC 61508 (functional safety of E/E/PE)"        "https://webstore.iec.ch/publication/5515"
    note_paywalled "IEC 62304 (medical device software)"           "https://webstore.iec.ch/publication/22794"
    note_paywalled "RTCA DO-178C / EUROCAE ED-12C (avionics SW)"   "https://my.rtca.org/productdetails?id=a1B36000001IcmwEAC"
    note_paywalled "EN 50128 / EN 50716 (railway SW)"              "https://www.cenelec.eu/"
    note_paywalled "ISO/IEC 19516:2020 (IDL)"                      "https://www.iso.org/standard/65064.html"
    log "(Die OMG-IDL-4.2-Spec ist inhaltsgleich zu ISO 19516 und frei bei OMG verfuegbar.)"
fi

info "Done."
log "Cache: $CACHE_DIR"
log "Inhalt pruefen:  find \"$CACHE_DIR\" -maxdepth 2 -type f | sort"
