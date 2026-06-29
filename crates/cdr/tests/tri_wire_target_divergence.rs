//! Tri-wire mapping contract: the SAME IDL type serialises to THREE different
//! wires depending on the target middleware. This test pins the divergences that
//! ZeroDDS's `type_map` resolves per target (DDS XCDR2 vs CORBA GIOP-CDR vs
//! ROS 2 rmw), so the documented mapping table cannot silently drift.
//!
//! Authority: the CORBA forms are byte-anchored to omniORB 4.3 + JacORB 3.9
//! (proofs in the campaign); the DDS form is the XCDR2 primitive wire; the ROS
//! form is REP-2007/2008 (Humble = XCDR1/PLAIN_CDR LE, wchar = UTF-32).
//!
//! See `internal/idl-codegen/conformance-surface-audit-2026-06.md` §D.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use zerodds_cdr::{BufferWriter, CdrEncode, Endianness, WChar};

/// `wchar` carrying U+20AC '€' maps three ways:
///   DDS   = `uint16` LE                       -> 2 bytes  AC 20
///   CORBA = GIOP 1.2 wchar (1-octet len + UTF-16 BE units, msg-order-independent)
///                                             -> 3 bytes  02 20 AC
///   ROS 2 = `uint32` UTF-32 LE                -> 4 bytes  AC 20 00 00
#[test]
fn wchar_euro_diverges_across_targets() {
    let scalar: u32 = 0x20AC; // '€'

    // --- DDS (XCDR2): wchar -> u16, little-endian ---
    let mut w = BufferWriter::new(Endianness::Little);
    (scalar as u16).encode(&mut w).expect("u16");
    assert_eq!(w.into_bytes(), vec![0xAC, 0x20], "DDS wchar = u16 LE");

    // --- CORBA (GIOP 1.2): real WChar codec, vendor-anchored (omniORB/JacORB) ---
    let mut w = BufferWriter::new(Endianness::Little);
    WChar('\u{20AC}').encode(&mut w).expect("wchar");
    assert_eq!(
        w.into_bytes(),
        vec![0x02, 0x20, 0xAC],
        "CORBA wchar = 1-octet len + UTF-16 BE (NOT message byte order)"
    );

    // --- ROS 2 (Humble): wchar -> u32 UTF-32, little-endian ---
    let mut w = BufferWriter::new(Endianness::Little);
    scalar.encode(&mut w).expect("u32");
    assert_eq!(
        w.into_bytes(),
        vec![0xAC, 0x20, 0x00, 0x00],
        "ROS wchar = u32 LE"
    );
}
