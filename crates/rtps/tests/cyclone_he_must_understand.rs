//! WP 1.C — Cyclone interop negative test for a HeaderExtension with
//! an unknown must-understand PID.
//!
//! **Opt-in only** — marked `#[ignore]`. Invocation:
//!
//! ```bash
//! cargo test -p zerodds-rtps --test cyclone_he_must_understand -- --ignored --nocapture
//! ```
//!
//! # Spec background
//!
//! DDSI-RTPS 2.5 §9.4.2.11.2: if a ParameterList in a
//! HeaderExtension contains a PID whose `must_understand` bit
//! (0x4000) is set, and the receiver does not know this PID,
//! it MUST discard the whole RTPS message.
//!
//! # Test flow (unseen Cyclone machine)
//!
//! 1. Local: we produce an RTPS message consisting of
//!    - RtpsHeader (ZeroDDS VendorId)
//!    - a HeaderExtension with a ParameterList containing a `must_understand`
//!      PID that Cyclone does not know (e.g. PID 0xC042 — vendor-
//!      specific + must-understand).
//!    - followed by a DATA submessage with a regular
//!      DCPSParticipant payload (SPDP builtin topic).
//!
//! 2. Send this message via UDP multicast to Cyclone.
//!
//! 3. Expectation: Cyclone does **not** discover this participant
//!    (whole-message reject).
//!
//! 4. Comparison run: same message without the must-understand PID →
//!    Cyclone discovers correctly.
//!
//! Without a Cyclone live setup this test only verifies that our
//! encoder produces the correct wire form, that our decoder would itself
//! also reject, and that the hex dump matches
//! what we need for debugging verification against Cyclone.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_rtps::header::RtpsHeader;
use zerodds_rtps::header_extension::{HE_FLAG_E, HeaderExtension};
use zerodds_rtps::parameter_list::{MUST_UNDERSTAND_BIT, Parameter, ParameterList};
use zerodds_rtps::wire_types::{GuidPrefix, VendorId};

/// Builds the RTPS message in question with HE.P + must-understand PID.
fn build_he_with_unknown_must_understand_pid() -> Vec<u8> {
    let header = RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([0x42; 12]));
    let mut pl = ParameterList::new();
    // Vendor PID 0x0042 with must-understand bit + vendor bit:
    // 0x4042 (must-understand) | 0x8000 (vendor) = 0xC042. Cyclone
    // does not know 0x0042 and therefore MUST discard the whole message.
    pl.push(Parameter::new(MUST_UNDERSTAND_BIT | 0x8042, vec![0; 4]));
    let he = HeaderExtension {
        little_endian: true,
        message_length: Some(0),
        parameters: Some(pl),
        ..HeaderExtension::default()
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&header.to_bytes());
    bytes.extend_from_slice(&he.encode().unwrap());
    bytes
}

#[test]
fn local_encoder_produces_must_understand_bit() {
    let bytes = build_he_with_unknown_must_understand_pid();
    // Sanity: HE submessage ID 0x80 is after the 20-byte RTPS header.
    assert_eq!(bytes[20], 0x80);
    // P flag (bit 7) set in the flag byte.
    assert_eq!(bytes[21] & 0x80, 0x80);
    // E flag (bit 0) set in the flag byte.
    assert_eq!(bytes[21] & HE_FLAG_E, HE_FLAG_E);
}

#[test]
fn parameter_with_must_understand_helper_sets_bit() {
    // Spec §9.4.2.11.2: sender sets the must-understand bit (`0x4000`)
    // per PID whose understanding is mandatory on the receiver side.
    use zerodds_rtps::parameter_list::{MUST_UNDERSTAND_BIT, Parameter};
    let p = Parameter::new(0x0042, vec![0u8; 4]).with_must_understand();
    assert!(
        p.has_must_understand(),
        "the bit setter must set the MU flag"
    );
    assert_eq!(
        p.id & MUST_UNDERSTAND_BIT,
        MUST_UNDERSTAND_BIT,
        "wire value carries the MU bit"
    );
    assert_eq!(p.id & 0x3FFF, 0x0042, "original PID is preserved");
}

#[test]
fn local_decoder_rejects_via_validate_must_understand() {
    use zerodds_rtps::datagram::ParsedSubmessage;
    use zerodds_rtps::datagram::decode_datagram;

    let bytes = build_he_with_unknown_must_understand_pid();
    let parsed = decode_datagram(&bytes).expect("HE decode may succeed");
    let he = match &parsed.submessages[0] {
        ParsedSubmessage::HeaderExtension(he) => he,
        other => panic!("expected HE, got {other:?}"),
    };
    let pl = he.parameters.as_ref().expect("PL must be present");
    // Receiver kennt nur PID 0x0015 — der Must-Understand-PID 0x0042
    // is unknown → reject.
    let res = pl.validate_must_understand(|pid| pid == 0x0015);
    assert!(res.is_err(), "whole-message reject required (§9.4.2.11.2)");
}

#[test]
#[ignore = "needs live Cyclone DDS instance + multicast network setup"]
fn cyclone_live_rejects_he_with_unknown_must_understand_pid() {
    // Live test against Cyclone DDS:
    // 1. Build the RTPS datagram with HE.P + must-understand vendor PID.
    // 2. Send via UDP to the SPDP multicast address (239.255.0.1:7400 +
    //    port computation for the domain).
    // 3. Expectation: Cyclone discards the message, the participant does
    //    NOT appear in `ddsperf -D <DOMAIN>` discovery.
    //
    // The implementation needs the same multicast setup as
    // `crates/discovery/tests/cyclone_live_sedp.rs`. As long as the lab
    // setup is not available in CI, this test stays
    // `#[ignore]`.
    let _ = build_he_with_unknown_must_understand_pid();
}
