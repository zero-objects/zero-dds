# Cyclone XCDR2 Wire-Fixtures

XCDR2 payloads from Cyclone DDS for V-1..V-12 of
`docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md` §6.

Each `v<N>.bin` is the raw XCDR2 payload (no RTPS header) emitted by
Cyclone's encoder for the corresponding sample.

**How each committed `.bin` was obtained is not recorded.** The section
below describes a capture procedure and says in the same breath that the
capture mode is not wired, so these files are the encoder's output unless
someone shows otherwise. For Cyclone the distinction is narrow — it
writes its own alignment fill (`dds_cdrstream.c:343`) and the payload is
what its RTPS layer sends — but "captured" is a stronger word than the
evidence here supports, and elsewhere in this tree that word has been
wrong: see `tools/wire-capture`.

## Recording

Use `tests/interop/capture.sh` with `--cdr xcdr2` once that capture
mode is wired. Until then:

1. Build a Cyclone DDS publisher with the V-N IDL.
2. Use `tcpdump`/`wireshark` to capture the RTPS DATA submessage.
3. Strip the RTPS header (16 bytes) plus the encoding-header (4 bytes)
   to leave the raw XCDR2 payload.
4. Save as `v<N>.bin` in this directory.

## Verification

`cargo test -p zerodds-cdr --test xcdr2_cross_vendor_fixtures` decodes
each fixture and asserts byte-identity with the master-spec hex.
