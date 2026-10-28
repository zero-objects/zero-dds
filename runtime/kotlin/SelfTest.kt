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

    if (failures != 0) {
        println("$failures check(s) failed")
        exitProcess(1)
    }
}
