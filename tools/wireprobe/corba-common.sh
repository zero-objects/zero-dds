# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The command line the three ORB probes share.
#
# It is the one `wireprobe-cyclone`, `wireprobe-rti`, `wireprobe-fastdds`
# and `wireprobe-zerodds` take — `--idl --type --values --encoding
# --endian --emit --out`, plus `--in` and `--values-out` for the read
# direction — so a case runs through eight implementations by
# substituting the program name.
#
# ## What an ORB cannot be asked
#
# `--encoding` accepts the XCDR1 forms and nothing else, and the refusal
# is loud. XCDR1 *is* CORBA CDR (CORBA 3.3 part 2 §9.3), so those are
# the encodings an ORB has an answer for; XCDR2 is XTypes 1.3 §7.4.2 and
# an ORB has never heard of it. A probe that took `--encoding xcdr2` and
# answered with CORBA CDR bytes would put a CDR payload in an XCDR2
# column of the matrix, where every reader of that matrix would take it
# for a divergence.
#
# `pl-cdr1` is refused with the rest. It is XCDR1's *parameter list*
# framing for a `@mutable` type — rule (23) — which is XTypes and not
# CORBA; the plain forms are the shared ones.
#
# `--emit` accepts `payload` only. CORBA CDR has no encapsulation for a
# top-level parameter: GIOP §9.4 carries the byte order in the message
# header, so there is no two-byte identifier for an ORB to write and
# `payload-with-encapsulation` and `keyhash` have no CORBA answer at all.
#
# `--dialect` is refused for anything but the probe's own ORB, the way
# the four DDS probes refuse it: this probe writes what its ORB writes.

# corba_parse_arguments <orb name> "$@"
#
# Sets: idl, type_name, values, input, values_out, out, big.
corba_parse_arguments() {
    local orb="$1"
    shift
    idl=""; type_name=""; values=""; encoding="xcdr1"; endian=""; emit="payload"
    out=""; input=""; values_out=""; matrix=0
    while [ $# -gt 0 ]; do
        case "$1" in
            # Every row an ORB can answer, out of the one build. CORBA
            # CDR is encoding version 1 and nothing else — CORBA 3.3
            # part 2 §9.3 — so the rows are the byte orders, and two of
            # the three ORBs write one each and say so for the other.
            --matrix) matrix=1; endian="${endian:-le}"; shift ;;
            --idl) idl="$2"; shift 2 ;;
            --type) type_name="$2"; shift 2 ;;
            --values) values="$2"; shift 2 ;;
            --in) input="$2"; shift 2 ;;
            --values-out) values_out="$2"; shift 2 ;;
            --encoding) encoding="$2"; shift 2 ;;
            --endian) endian="$2"; shift 2 ;;
            --emit) emit="$2"; shift 2 ;;
            --out) out="$2"; shift 2 ;;
            --dialect)
                if [ "$2" != "$orb" ]; then
                    echo "wireprobe-$orb: --dialect $2 is not this probe's; it writes what $orb writes" >&2
                    exit 3
                fi
                shift 2 ;;
            # Accepted and ignored: these select between shapes this
            # project can produce, and an ORB has exactly one of each.
            # Ignoring them lets the same command line drive every probe.
            --length-codes|--collection-delimiters|--map-length|--wstring-length) shift 2 ;;
            --pl-cdr1-headers|--default-extensibility|--key-holder) shift 2 ;;
            --help|-h) sed -n '2,30p' "$0"; exit 0 ;;
            *) echo "wireprobe-$orb: \`$1\` is not an option" >&2; exit 2 ;;
        esac
    done

    [ -n "$idl" ] || { echo "wireprobe-$orb: --idl is required" >&2; exit 2; }
    [ -n "$type_name" ] || { echo "wireprobe-$orb: --type is required" >&2; exit 2; }
    if [ -n "$input" ] && [ -n "$values" ]; then
        echo "wireprobe-$orb: --in reads bytes and --values writes them; pick one" >&2; exit 2
    fi
    if [ -z "$input" ] && [ -z "$values" ]; then
        echo "wireprobe-$orb: --values is required, or --in to read bytes back" >&2; exit 2
    fi

    case "$encoding" in
        xcdr1|plain-cdr1) ;;
        pl-cdr1)
            echo "wireprobe-$orb: pl-cdr1 is XTypes 1.3 rule (23)'s parameter list, which no ORB has" >&2
            exit 3 ;;
        xcdr2|plain-cdr2|delimited-cdr|pl-cdr2)
            echo "wireprobe-$orb: \`$encoding\` is XTypes 1.3 §7.4.2 encoding version 2, which no ORB has" >&2
            exit 3 ;;
        *) echo "wireprobe-$orb: \`$encoding\` is not an encoding" >&2; exit 2 ;;
    esac

    case "$emit" in
        payload) ;;
        payload-with-encapsulation)
            echo "wireprobe-$orb: CORBA CDR has no encapsulation for a top-level parameter; GIOP §9.4 carries the byte order in the message header" >&2
            exit 3 ;;
        keyhash)
            echo "wireprobe-$orb: a KeyHash is DDSI-RTPS §9.6.3.8 and has no CORBA form" >&2
            exit 3 ;;
        *) echo "wireprobe-$orb: --emit $emit is not an emission" >&2; exit 2 ;;
    esac

    case "$endian" in
        "") echo "wireprobe-$orb: --endian is required" >&2; exit 2 ;;
        le|little) big=0; endian_word=le ;;
        be|big) big=1; endian_word=be ;;
        *) echo "wireprobe-$orb: \`$endian\` is not a byte order" >&2; exit 2 ;;
    esac
}
