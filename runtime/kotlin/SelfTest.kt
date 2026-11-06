// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Kotlin wire runtime's self-test for the three axes
// `Dialect::Hdds` selects.
//
// Silent and exit 0 is the pass, as in `runtime/c/tests/selftest.c` and
// `runtime/java/src/test/java/org/zerodds/wire/WireSelfTest.java`. Every
// expectation here is a byte string, not a round trip: a writer and a reader
// that share a misreading of §7.4.1 agree with each other perfectly.
//
// Expected octets from the recorded HDDS columns:
// tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin,
// tests/interop-results/typedef-of-struct/hdds/plain-cdr2.le.bin,
// tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin,
// tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
// tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
// tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin.

package zerodds.wire

import kotlin.system.exitProcess

private var failures = 0

private fun hex(bytes: ByteArray): String =
    bytes.joinToString("") { byte -> "%02x".format(byte.toInt() and 0xFF) }

private fun expect(got: String, want: String, what: String) {
    if (got != want) {
        println("FAIL $what: $got is not $want")
        failures++
    }
}

private fun expectThrows(body: () -> Unit, what: String) {
    try {
        body()
    } catch (error: WireException) {
        return
    }
    println("FAIL $what: nothing was raised")
    failures++
}

fun main() {
    // ElementPadding::Packed: each collection element counts its own
    // alignment from its own first octet, so the second Cell's long lands at
    // offset six and not eight.
    val packed = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    var saved = packed.beginPackedElement()
    packed.writeI32(1)
    packed.writeI16(2)
    packed.finishPackedElement(saved)
    saved = packed.beginPackedElement()
    packed.writeI32(3)
    packed.writeI16(4)
    packed.finishPackedElement(saved)
    packed.writeU8(119)
    expect(hex(packed.finish()), "01000000020003000000040077",
        "a packed array element starts where the last ended")

    val packedBack = Reader(packed.finish(), Endianness.LITTLE, 4, PlCdr1Framing.specification())
    saved = packedBack.readerBeginPackedElement()
    val a1 = packedBack.readI32()
    val b1 = packedBack.readI16()
    packedBack.readerFinishPackedElement(saved)
    saved = packedBack.readerBeginPackedElement()
    val a2 = packedBack.readI32()
    val b2 = packedBack.readI16()
    packedBack.readerFinishPackedElement(saved)
    expect("$a1,$b1,$a2,$b2,${packedBack.readU8()}", "1,2,3,4,119",
        "a packed element reads back at its own origin")

    // The same inside two @final members.
    val aggregate = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    saved = aggregate.beginPackedElement()
    aggregate.writeI32(-1234567)
    aggregate.writeI16(-2)
    aggregate.finishPackedElement(saved)
    saved = aggregate.beginPackedElement()
    aggregate.writeI32(1)
    aggregate.writeI16(32767)
    aggregate.finishPackedElement(saved)
    expect(hex(aggregate.finish()), "7929edfffeff01000000ff7f",
        "a packed aggregate counts alignment from its own start")

    // A sequence element's own length in DHEADER form: the 06000000 in front
    // of 7929edfffeff is the element's six octets, not the encapsulation's.
    val element = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    val header = element.writeDheader()
    element.writeI32(-1234567)
    element.writeI16(-2)
    element.finishDheader(header)
    expect(hex(element.finish()), "060000007929edfffeff",
        "a sequence element carries its own length")

    // WstringCodeUnits::Utf8: the characters' own octets, the octet count with
    // the terminator, and C2 still reached.
    val text = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    text.writeWstringUtf8Octets("abc", WstringLengthPolicy.OCTETS_TERMINATED_2)
    expect(hex(text.finish()), "0400000061626300", "a UTF-8 wstring is its octets and a NUL")

    val multibyte = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    multibyte.writeWstringUtf8Octets("äöü ሴ Z", WstringLengthPolicy.OCTETS_TERMINATED_2)
    expect(hex(multibyte.finish()), "0d000000c3a4c3b6c3bc20e188b4205a00",
        "a multi-byte UTF-8 wstring counts its octets")

    val bounded = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    bounded.writeWstringUtf8OctetsBounded("ሴäZ", 3, WstringLengthPolicy.OCTETS_TERMINATED_2)
    expect(hex(bounded.finish()), "07000000e188b4c3a45a00",
        "a bounded UTF-8 wstring holds three units")

    val record = byteArrayOf(
        0x0d, 0x00, 0x00, 0x00, 0xc3.toByte(), 0xa4.toByte(), 0xc3.toByte(), 0xb6.toByte(),
        0xc3.toByte(), 0xbc.toByte(), 0x20, 0xe1.toByte(), 0x88.toByte(), 0xb4.toByte(),
        0x20, 0x5a, 0x00,
    )
    val textBack = Reader(record, Endianness.LITTLE, 4, PlCdr1Framing.specification())
    expect(textBack.readWstringUtf8Octets(WstringLengthPolicy.OCTETS_TERMINATED_2),
        "äöü ሴ Z", "a UTF-8 wstring reads back as its octets")

    // Malformed UTF-8 and a count the stream cannot hold are refused, never
    // replaced or truncated.
    expectThrows({
        Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
            .writeWstringUtf8Octets("\uD834", WstringLengthPolicy.OCTETS_TERMINATED_2)
    }, "a lone surrogate is not written as UTF-8 octets")
    expectThrows({
        val impossible = Reader(
            byteArrayOf(0xff.toByte(), 0x00, 0x00, 0x00, 0x61, 0x00),
            Endianness.LITTLE, 4, PlCdr1Framing.specification())
        impossible.readWstringUtf8Octets(WstringLengthPolicy.OCTETS_TERMINATED_2)
    }, "a wstring length past the buffer")

    // -- the operations no corpus case reaches -------------------------------
    //
    // The five the generated code reaches only in a dialect or with a member
    // no corpus case carries: writePidExtended, writeUnionOverlay,
    // readerClearDiscard, readerSave and readerRestore
    // (internal/release/OPEN-ITEMS.md). Every runtime checks the same four
    // vectors, each against an oracle outside this code.

    // The first parameter of `annotation-autoid-sequential` as Cyclone DDS
    // recorded it, XCDR1 little endian (tests/interop-corpus/expected-bytes.tsv).
    val pid = Writer(Endianness.LITTLE, 8, PlCdr1Framing.specification(), false)
    val pidHeader = pid.writePidExtended(0, ParameterRole.MEMBER, false)
    pid.writeI32(-1234567)
    pid.finishPid(pidHeader)
    expect(hex(pid.finish()), "017f0800" + "00000000" + "04000000" + "7929edff",
        "writePidExtended: Cyclone's recorded parameter")

    // A `short` read through a four-octet and a one-octet `default:` branch:
    // Cyclone DDS 11.0.1 writes the union's storage, the member's octets with
    // zeros above (`combined-everything`, crates/wire-codec/src/encode.rs
    // `write_selected_branch`).
    val overlay = Writer(Endianness.LITTLE, 4, PlCdr1Framing.specification(), false)
    val pattern = bitPatternInt(-2L, 2)
    overlay.writeUnionOverlay(pattern, 4)
    overlay.writeUnionOverlay(pattern, 1)
    expect(hex(overlay.finish()), "feff0000" + "fe",
        "writeUnionOverlay: the storage at the branch's width")

    // XTypes 1.3 §7.2.2.4.4.4.9: a member's explicit @default comes before
    // Table 9's value, so the call site asks for DISCARD and takes it back.
    // Any other refusal is not a narrowing and stays.
    val four = byteArrayOf(1, 2, 3, 4)
    val discarded = Reader(four, Endianness.LITTLE, 4, PlCdr1Framing.specification())
    discarded.readerRefuse(Status.DISCARDED)
    expect(discarded.readerClearDiscard().toString(), "true", "readerClearDiscard takes back a DISCARDED")
    expect(discarded.readerStatus().text(), "ok", "the status after readerClearDiscard")
    val refused = Reader(four, Endianness.LITTLE, 4, PlCdr1Framing.specification())
    refused.readerRefuse(Status.OUT_OF_RANGE)
    expect(refused.readerClearDiscard().toString(), "false", "readerClearDiscard leaves an OUT_OF_RANGE")
    expect(refused.readerStatus().text(), "out-of-range", "an OUT_OF_RANGE stays")

    // XTypes 1.3 §7.4.3.5.3 rule (15): a delimited map<long, long> as its
    // DHEADER and the pairs, without a count. The reader counts the pairs,
    // goes back and reads them again; a refusal made while counting goes back
    // with the cursor.
    val map = byteArrayOf(0x10, 0, 0, 0, 1, 0, 0, 0, 0x0a, 0, 0, 0, 2, 0, 0, 0, 0x14, 0, 0, 0)
    val reader = Reader(map, Endianness.LITTLE, 4, PlCdr1Framing.specification())
    val extent = reader.readDheader()
    val end = reader.readerPosition() + extent.toInt()
    val mark = reader.readerSave()
    var pairs = 0
    while (reader.readerPosition() < end && reader.readerStatus() == Status.OK) {
        reader.readI32()
        reader.readI32()
        pairs++
    }
    reader.readerRefuse(Status.OUT_OF_RANGE)
    reader.readerRestore(mark)
    expect(pairs.toString(), "2", "the counting pass finds two pairs")
    expect(listOf(reader.readI32(), reader.readI32(), reader.readI32(), reader.readI32()).joinToString(","),
        "1,10,2,20", "the pairs read again")
    expect(reader.readerPosition().toString(), "20", "the end of the map")
    expect(reader.readerStatus().text(), "ok", "readerRestore takes the refusal back")

    if (failures != 0) {
        println("$failures check(s) failed")
        exitProcess(1)
    }
}
