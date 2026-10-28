// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every dialect, both directions, and what happens when they differ.
//!
//! The read direction takes the same flags as the write direction:
//! `--dialect rti` means "read what RTI writes" exactly as it means
//! "write what RTI writes". So each case below goes out under one
//! dialect and comes back under every dialect, and the two halves are
//! asserted separately:
//!
//! 1. **Its own dialect returns the sample.** Byte for byte out, value
//!    for value back, with nothing left in the frame.
//! 2. **Another dialect does not return a wrong sample.** Either the
//!    read is refused with an error that names what is wrong, or the
//!    two dialects produce the same bytes for this case and there is
//!    nothing to tell apart. Which of the two, per pair, is asserted
//!    rather than left open: a case that quietly moved from the first
//!    group to the third would otherwise read as coverage.
//!
//! The third outcome — a different sample, returned as if it were the
//! one that was sent — is the defect this file exists to keep out. It
//! was live until this week: a Fast DDS `wstring` read under the
//! specification's length form came back as half the string with the
//! next member's first byte in it.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast_layout::{Dialect, Encoding, Endianness, PidFlagPolicy};
use zerodds_wire_probe::canonical::{canonicalise, text};
use zerodds_wire_probe::decode::decode;
use zerodds_wire_probe::encode::{encode, hex, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// Whether the difference between two dialects is visible in the
/// stream, and therefore whether a reader in the wrong dialect can be
/// expected to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detectable {
    /// Nothing in the bytes says which dialect wrote them. A reader in
    /// the wrong one has to refuse rather than return a value: the
    /// framing it expects is not the framing it was handed, and reading
    /// on would return a sample nobody sent.
    OnlyFromTheSetting,
    /// The difference is in a field the reader reads. An EMHEADER
    /// carries its own length code and a member id carries its own
    /// must-understand bit, so a stream written under one policy reads
    /// correctly under another — §7.4.3.4.2 exists to make exactly that
    /// true. Here the *right* answer is the same sample from different
    /// bytes, and asserting a refusal would be asserting a defect.
    FromTheStream,
}

/// Whether the difference between two dialects, for this case, is one
/// no reader could see.
///
/// The third answer, and the one the CORBA column forced. A `wstring`'s
/// code-unit byte order is not in the stream: XTypes §7.4.1.1.2 makes
/// the units the stream's, the OMG code-set rule for a GIOP 1.2
/// `wstring` makes them UTF-16's own, and `04000000 61006200` is a
/// legal encoding of `"ab"` under the first and of `"愀戀"` under the
/// second. Each side is self-consistent and nothing in the frame says
/// which. omniORB's byte-order mark is exactly an implementation
/// noticing that and fixing it for its own reader.
///
/// So for these pairs a wrong sample is not a defect in either reader —
/// it is the measured divergence, and it is the reason
/// `WstringUnitOrder` is a setting rather than a detection. The test
/// counts them and holds a floor, so that the undecidability stays
/// pinned instead of being quietly assumed away; every other pair keeps
/// the strict rule that a cross-dialect read refuses.
type Undecidable = fn(Dialect, Dialect) -> bool;

/// The default: every difference this case has is one a reader can see
/// or refuse.
fn always_decidable(_writer: Dialect, _reader: Dialect) -> bool {
    false
}

/// The wide-character family, which is the part of the wire no stream
/// describes.
///
/// Three axes and all three are settings for the same reason. The
/// **length convention** — `04000000 61006200` is `"ab"` as a byte count
/// and `"愀戀"` under a different unit order and `"a\0b\0"` read two
/// octets at a time out of TAO's four. The **code-unit width** — a byte
/// count that four divides is a legal byte count over two-octet units
/// too. The **scalar framing** — a bare `Char16` carries no length, so
/// nothing distinguishes it from an octet count that happens to hold a
/// character's first byte.
///
/// [`zerodds_ast_layout::WstringLengthPolicy`]'s own documentation
/// states the position this predicate encodes: a reader choosing between
/// the forms "would not be resolving an ambiguity, it would be inventing
/// an answer, and it would be taking it from the length field — the
/// field a hostile peer controls".
///
/// The pairs are still decoded and still classified. What is dropped is
/// the demand that they *refuse*, which for these is a demand that a
/// reader detect something the bytes do not contain.
fn only_the_wide_character_settings(writer: Dialect, reader: Dialect) -> bool {
    writer.wstring_unit_order() != reader.wstring_unit_order()
        || writer.wchar_framing() != reader.wchar_framing()
        || writer.wstring_length(Encoding::PlainCdr1) != reader.wstring_length(Encoding::PlainCdr1)
}

/// Whether, for this case, `reader`'s framing accepts what `writer`
/// wrote **by design**, so that the right answer is the same sample from
/// different bytes.
///
/// The fourth answer, and the `PID_EXTENDED` marker forced it. XTypes
/// 1.3 §7.4.1.2.3 makes the extended parameter header mandatory above
/// the short form's limits and not exclusive to them, so a reader that
/// does not *require* the extended form still has to read one. Cyclone
/// writes the extended form for every member; RTI and the specification
/// read it, and since the marker's flags moved from
/// `PlCdr1HeaderPolicy` to `PidFlagPolicy` they expect the same `7f01`
/// on it, so nothing in a Cyclone parameter list is outside what they
/// accept.
///
/// It is not [`Undecidable`]: the bytes differ and a reader could in
/// principle refuse them. It is that refusing would be the defect.
type ReadsAlike = fn(Dialect, Dialect) -> bool;

/// The default: no pair of framings accepts the other's bytes.
fn no_pair_reads_alike(_writer: Dialect, _reader: Dialect) -> bool {
    false
}

/// Pairs whose difference is a framing one side **omits** rather than
/// writes with different octets.
///
/// `Dialect::Hdds` frames every aggregate as `@final` (no DHEADER and no
/// EMHEADER), packs a collection's elements, writes a `sequence`
/// element's own length in front of it, and writes a `wstring` in UTF-8.
/// XTypes 1.3 §7.4.3.5.3 puts each of those framings in the octets, so
/// the clause's reader meets a stream whose first member is not the
/// header it expects: the difference is not a value in the stream that a
/// reader could read *around*, it is the absence of a structure a reader
/// would have to know the writer had left out.
///
/// So such a pair is undecidable in the same sense as an unswapped wide
/// unit: it is counted in `wide_refused`/`wide_silent` and not required
/// to read back, either as the sample or as a refusal that names the
/// setting. Every other pair is unaffected: the four axes are equal for
/// all dialects but `Hdds`, so this is `false` unless one side is
/// `Hdds`.
fn an_omitted_framing_is_not_in_the_stream(
    writer: Dialect,
    reader: Dialect,
    encoding: Encoding,
) -> bool {
    writer.aggregate_delimiter(encoding) != reader.aggregate_delimiter(encoding)
        || writer.element_padding() != reader.element_padding()
        || writer.sequence_element_delimiter() != reader.sequence_element_delimiter()
        || writer.wstring_code_units() != reader.wstring_code_units()
}

/// The always-extended writer against every reader that does not demand
/// the short form.
///
/// One direction only. The reverse — a short header read by a reader
/// that requires the extended form — is refused, and the strict rule
/// still covers it.
fn the_extended_form_is_read_by_every_form(writer: Dialect, reader: Dialect) -> bool {
    writer.pl_cdr1_headers().always_extended()
        && !reader.pl_cdr1_headers().always_extended()
        && writer.pid_flags() == reader.pid_flags()
        && writer.pl_cdr1_headers().pads_declared_length()
            == reader.pl_cdr1_headers().pads_declared_length()
}

/// Two framings that differ only in the **flags** on a member's
/// parameter id.
///
/// §7.4.1.2.2 puts `PID_IMPL_EXTENSION` and `PID_MUST_UNDERSTAND` in the
/// id's high bits and the member id in the low fourteen, and a reader
/// masks them off before it looks the member up. So Fast DDS's
/// `00000004` and OpenDDS's `c0000004` are the same member, and a
/// reader of either that refused the other's stream would be refusing a
/// difference the clause says it must tolerate.
///
/// **One direction only**, and the other direction is the strict rule
/// working. A reader whose framing sets the two bits masks them off and
/// reads a bare id as the member it names; a reader whose framing does
/// not set them meets `PID_IMPL_EXTENSION` and `PID_MUST_UNDERSTAND` on
/// a parameter it did not expect to carry either, and §7.4.1.2.2 says
/// what that means — a parameter of the writer's own that the reader
/// must understand and does not. Refusing is the answer, and
/// `Ext: expected a member the type declares, found member id 0 with
/// must_understand set` is that refusal.
///
/// The pair only appeared when OpenDDS got a dialect: the flags were a
/// setting nothing selected, so no two presets differed in them alone.
fn the_flags_on_a_member_id_are_not_part_of_it(writer: Dialect, reader: Dialect) -> bool {
    writer.pl_cdr1_headers() == reader.pl_cdr1_headers()
        && writer.pid_flags() != PidFlagPolicy::ImplExtensionOnMembers
        && reader.pid_flags() == PidFlagPolicy::ImplExtensionOnMembers
}

/// Either of the two reasons a `@mutable` XCDR1 stream reads under
/// another framing.
fn a_mutable_xcdr1_stream_reads_alike(writer: Dialect, reader: Dialect) -> bool {
    the_extended_form_is_read_by_every_form(writer, reader)
        || the_flags_on_a_member_id_are_not_part_of_it(writer, reader)
}

/// One IDL type, one sample, one encoding.
struct Case {
    /// What the case is for, in the failure message.
    what: &'static str,
    idl: &'static str,
    type_name: &'static str,
    encoding: Encoding,
    values: fn() -> Value,
    /// What a read under another dialect must do with these bytes.
    across: Detectable,
    /// Which pairs are exempt, because their difference is not in the
    /// bytes at all. See [`Undecidable`].
    undecidable: Undecidable,
    /// Which pairs the reader accepts by design. See [`ReadsAlike`].
    reads_alike: ReadsAlike,
}

fn cases() -> Vec<Case> {
    vec![
        // The PL_CDR1 framing split: three header forms, three
        // declared lengths for the trailing `short`, two sentinels.
        Case {
            what: "a mutable struct under XCDR1",
            idl: "@mutable struct Ext { long a; double b; short c; };",
            type_name: "Ext",
            encoding: Encoding::PlCdr1,
            values: || json!({ "a": 7, "b": 1.5, "c": -3 }),
            across: Detectable::OnlyFromTheSetting,
            undecidable: always_decidable,
            reads_alike: a_mutable_xcdr1_stream_reads_alike,
        },
        // The default-extensibility split: issues #41 and #42.
        Case {
            what: "an unannotated aggregate under XCDR2",
            idl: "\
struct Location { double x; double y; double z; };
struct Robot { Location spot; long tail; };",
            type_name: "Robot",
            encoding: Encoding::DelimitedCdr,
            values: || json!({ "spot": { "x": 1.0, "y": 2.0, "z": 3.0 }, "tail": -1 }),
            across: Detectable::OnlyFromTheSetting,
            undecidable: always_decidable,
            reads_alike: no_pair_reads_alike,
        },
        // The collection-delimiter split: Cyclone delimits an enum
        // collection, Fast DDS does not, RTI delimits nothing.
        Case {
            what: "a sequence of enumerations under XCDR2",
            idl: "\
enum Colour { RED, GREEN, BLUE };
@final struct SeqE { sequence<Colour> cs; long tail; };",
            type_name: "SeqE",
            encoding: Encoding::PlainCdr2,
            values: || json!({ "cs": ["BLUE", "RED"], "tail": -1 }),
            across: Detectable::OnlyFromTheSetting,
            undecidable: always_decidable,
            reads_alike: no_pair_reads_alike,
        },
        // The wstring split: byte count, character count, and RTI's
        // NUL-terminated four-byte characters under XCDR1.
        Case {
            what: "a wstring under XCDR1",
            idl: "@final struct WStr { wstring w; octet tail; };",
            type_name: "WStr",
            encoding: Encoding::PlainCdr1,
            values: || json!({ "w": "ab", "tail": 171 }),
            across: Detectable::OnlyFromTheSetting,
            undecidable: only_the_wide_character_settings,
            reads_alike: no_pair_reads_alike,
        },
        // The scalar `wchar` split, which is the same setting seen at
        // its own width: RTI writes four bytes under XCDR1. The leading
        // octet is there so the widths do not coincide — without it
        // both forms start at offset 0 and end at offset 4, and the two
        // dialects write the same bytes.
        Case {
            what: "a wchar under XCDR1",
            idl: "@final struct WCh { octet head; wchar c; long n; };",
            type_name: "WCh",
            encoding: Encoding::PlainCdr1,
            values: || json!({ "head": 1, "c": "a", "n": 7 }),
            across: Detectable::OnlyFromTheSetting,
            undecidable: only_the_wide_character_settings,
            reads_alike: no_pair_reads_alike,
        },
        // The length-code split: Cyclone writes LC4 for a nested
        // aggregate and for an eight-byte array, RTI reuses the nested
        // DHEADER, Fast DDS does that and shortens the array too.
        Case {
            what: "the length codes of a mutable struct under XCDR2",
            idl: "\
@final struct Cell { long a; long b; };
@appendable struct ACell { long a; long b; };
@mutable struct LenNest { ACell app; long ap[2]; Cell nested; long tail; };",
            type_name: "LenNest",
            encoding: Encoding::PlCdr2,
            values: || {
                json!({
                    "app": { "a": 1, "b": 2 }, "ap": [7, 8],
                    "nested": { "a": 9, "b": 10 }, "tail": 11
                })
            },
            across: Detectable::FromTheStream,
            undecidable: always_decidable,
            reads_alike: no_pair_reads_alike,
        },
        // The must-understand split, which is one property in three
        // places: Fast DDS writes the flag in none of them, under both
        // encoding versions.
        Case {
            what: "a must-understand member under XCDR2",
            idl: "@mutable struct MU { @id(1) @must_understand long a; @id(2) long b; };",
            type_name: "MU",
            encoding: Encoding::PlCdr2,
            values: || json!({ "a": 7, "b": -2 }),
            across: Detectable::FromTheStream,
            undecidable: always_decidable,
            reads_alike: no_pair_reads_alike,
        },
    ]
}

fn request(case: &Case, dialect: Dialect, endianness: Endianness) -> ProbeRequest {
    ProbeRequest::in_dialect(case.type_name, case.encoding, endianness, dialect)
}

/// The sample as the probe canonicalises it, which is what a decode
/// returns and therefore what a decode has to be compared against.
fn canonical(case: &Case) -> String {
    let ast = model_from_idl(case.idl, "dialect.idl").expect("the model");
    let type_ = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            case.type_name.to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type");
    let values = (case.values)();
    text(&canonicalise(&ast, type_, &values, case.type_name).expect("the sample"))
}

/// What a dialect writes for a case, in both byte orders.
///
/// Both, because one of the four settings only shows in big endian and
/// because the byte-swap arms of the writer are otherwise unexecuted.
#[test]
fn every_dialect_reads_back_what_it_wrote() {
    for case in cases() {
        let ast = model_from_idl(case.idl, "dialect.idl").expect("the model");
        let sample = (case.values)();
        let expected = canonical(&case);
        for dialect in Dialect::ALL {
            for endianness in Endianness::ALL {
                let request = request(&case, dialect, endianness);
                let bytes = encode(&ast, &request, &sample).unwrap_or_else(|error| {
                    panic!("{} / {dialect:?} / {endianness:?}: {error}", case.what)
                });
                let read = decode(&ast, &request, &bytes).unwrap_or_else(|error| {
                    panic!(
                        "{} / {dialect:?} / {endianness:?}: its own bytes did not read \
                         back: {error}\n  {}",
                        case.what,
                        hex(&bytes)
                    )
                });
                assert_eq!(
                    text(&read),
                    expected,
                    "{} / {dialect:?} / {endianness:?}: the value came back different",
                    case.what
                );
            }
        }
    }
}

/// Every ordered pair of dialects, over every case.
///
/// Three outcomes are legitimate and the case says which one it is:
///
/// - the two dialects write the same bytes, so there is nothing to tell
///   apart;
/// - the difference is not in the stream, and the wrong dialect must
///   refuse rather than return a value — [`Detectable::OnlyFromTheSetting`];
/// - the difference *is* in the stream, and the wrong dialect must
///   return the right value — [`Detectable::FromTheStream`]. An EMHEADER
///   carries its own length code; §7.4.3.4.2 exists so that a reader
///   need not know which one the writer would have picked.
///
/// The fourth outcome, a different sample returned as if it were the one
/// that was sent, is the defect this file exists to keep out.
#[test]
fn a_stream_read_under_another_dialect_is_an_error_and_never_a_wrong_sample() {
    let mut identical = 0usize;
    let mut refused = 0usize;
    let mut self_describing = 0usize;
    let mut wide_refused = 0usize;
    let mut wide_silent = 0usize;
    let mut wrong: Vec<String> = Vec::new();

    for case in cases() {
        let ast = model_from_idl(case.idl, "dialect.idl").expect("the model");
        let sample = (case.values)();
        let expected = canonical(&case);
        for writer in Dialect::ALL {
            let wrote = request(&case, writer, Endianness::Little);
            let bytes = encode(&ast, &wrote, &sample).expect("the probe");
            for reader in Dialect::ALL {
                if reader == writer {
                    continue;
                }
                let reads = request(&case, reader, Endianness::Little);
                let theirs = encode(&ast, &reads, &sample).expect("the probe");
                if theirs == bytes {
                    identical += 1;
                    continue;
                }
                if (case.across == Detectable::FromTheStream
                    && !an_omitted_framing_is_not_in_the_stream(writer, reader, case.encoding))
                    || (case.reads_alike)(writer, reader)
                {
                    match decode(&ast, &reads, &bytes) {
                        Ok(read) if text(&read) == expected => self_describing += 1,
                        Ok(read) => wrong.push(format!(
                            "{}: {writer:?}'s bytes read as {reader:?} returned {} from \
                             a difference the stream carries",
                            case.what,
                            text(&read)
                        )),
                        Err(error) => wrong.push(format!(
                            "{}: {writer:?}'s bytes read as {reader:?} were refused, and \
                             the difference is one the header states: {error}",
                            case.what
                        )),
                    }
                    continue;
                }
                if (case.undecidable)(writer, reader)
                    || an_omitted_framing_is_not_in_the_stream(writer, reader, case.encoding)
                {
                    // The difference is not in the bytes. Whatever comes
                    // back is a property of two self-consistent readings
                    // and not of a defect — but *which* it is, is worth
                    // counting: the readers catch a great many of these
                    // anyway, on a length no other form could have
                    // written, and a change that stopped them catching
                    // any would move the second floor below.
                    match decode(&ast, &reads, &bytes) {
                        Err(_) => wide_refused += 1,
                        Ok(_) => wide_silent += 1,
                    }
                    continue;
                }
                match decode(&ast, &reads, &bytes) {
                    Err(_) => refused += 1,
                    Ok(read) if text(&read) == expected => wrong.push(format!(
                        "{}: {writer:?}'s bytes read as {reader:?} returned the right \
                         sample from the wrong bytes",
                        case.what
                    )),
                    Ok(read) => wrong.push(format!(
                        "{}: {writer:?}'s bytes read as {reader:?} returned {} instead \
                         of an error",
                        case.what,
                        text(&read)
                    )),
                }
            }
        }
    }

    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    // The count is asserted so that a change which made every pair
    // produce identical bytes — a preset collapsing into the default,
    // say — fails here instead of passing with nothing exercised.
    // The floors are here so that a preset collapsing into the default
    // fails the test instead of passing it with nothing exercised.
    assert!(
        refused >= 40,
        "only {refused} cross-dialect read(s) were refused, {identical} pair(s) \
         wrote the same bytes; the presets have stopped differing"
    );
    // The wide-character family is where a reader cannot always tell,
    // and both halves are held: it must still refuse what it *can*
    // refuse, and the cases it cannot are named rather than assumed.
    assert!(
        wide_refused >= 20,
        "only {wide_refused} wide-character cross-dialect read(s) were refused, with \
         {wide_silent} coming back silently; a reader that stopped refusing an \
         impossible length is not undecidable, it is broken"
    );
    assert!(
        wide_silent >= 5,
        "only {wide_silent} wide-character cross-dialect read(s) came back with a \
         different sample; the `wstring` unit-order and width presets have stopped \
         differing, which would mean `Dialect::OmniOrb`, `Dialect::Tao` and \
         `Dialect::JacOrb` no longer reproduce their ORBs"
    );
    assert!(
        self_describing >= 8,
        "only {self_describing} cross-dialect read(s) came back right from a \
         difference the header states; the length-code and must-understand \
         presets have stopped differing"
    );
}

/// The error a wrong dialect gives names a setting, not just an offset.
///
/// A user who picked the wrong flag has to be told which flag. The
/// probe's own errors carry the path; the framing and length-form ones
/// carry the setting the bytes do fit.
#[test]
fn the_error_from_a_wrong_dialect_names_what_would_fit() {
    let idl = "@final struct WStr { wstring w; octet tail; };";
    let ast = model_from_idl(idl, "dialect.idl").expect("the model");
    let sample = json!({ "w": "abc", "tail": 171 });

    // Fast DDS writes the character count, 3, where the specification
    // writes the byte count, 6. Three is not a byte count.
    let fastdds = ProbeRequest::in_dialect(
        "WStr",
        Encoding::PlainCdr1,
        Endianness::Little,
        Dialect::FastDds,
    );
    let bytes = encode(&ast, &fastdds, &sample).expect("the probe");
    let spec = ProbeRequest::in_dialect(
        "WStr",
        Encoding::PlainCdr1,
        Endianness::Little,
        Dialect::Specification,
    );
    let error = decode(&ast, &spec, &bytes).expect_err("a refusal");
    let text = format!("{error}");
    assert!(
        text.contains("--wstring-length"),
        "the error does not name the setting that fits: {text}"
    );

    // And the PL_CDR1 framing, where the setting is the header policy.
    let idl = "@mutable struct Ext { long a; short c; };";
    let ast = model_from_idl(idl, "dialect.idl").expect("the model");
    let sample = json!({ "a": 7, "c": -3 });
    let rti = ProbeRequest::in_dialect("Ext", Encoding::PlCdr1, Endianness::Little, Dialect::Rti);
    let bytes = encode(&ast, &rti, &sample).expect("the probe");
    let fastdds = ProbeRequest::in_dialect(
        "Ext",
        Encoding::PlCdr1,
        Endianness::Little,
        Dialect::FastDds,
    );
    let error = decode(&ast, &fastdds, &bytes).expect_err("a refusal");
    let text = format!("{error}");
    assert!(
        text.contains("--pl-cdr1-headers"),
        "the error does not name the setting that fits: {text}"
    );
}
