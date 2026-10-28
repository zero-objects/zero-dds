// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Kotlin endpoint SDK (ADR 0013): a from-scratch XCDR wire-core on
// the JVM, byte-identical to the Rust core and the other SDKs. Sync (blocking)
// and async (a background receive thread + a blocking-queue channel).

package zerodds

import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.LinkedBlockingQueue

enum class Endian { LITTLE, BIG }

// The XCDR version the stream is written in.
//
// It is a constructor argument and has no default. XTypes 1.3 §7.4.1 puts
// MAXALIGN at eight under XCDR1 and four under XCDR2, so a writer that did not
// know which one it was in would be writing XCDR2 and calling it XCDR — which
// is what this SDK did until the alignment cap became a field: `alignTo` read
// `if (a > 4) 4 else a`, and every `long long` in an XCDR1 sample landed four
// octets early with nothing in the interface to say so.
enum class Encoding(val maxAlign: Int) {
    // PLAIN_CDR/PL_CDR — MAXALIGN 8.
    XCDR1(8),

    // PLAIN_CDR2/DELIMITED_CDR/PL_CDR2 — MAXALIGN 4.
    XCDR2(4),
}

// The member header length codes of XTypes 1.3 §7.4.3.4.2 Table 34. The code
// occupies bits 28..30 of the EMHEADER and decides how the member's byte length
// is recovered: LC0..LC3 give it from the code alone, LC4..LC7 read it from a
// NEXTINT that follows the header.
const val LC0 = 0
const val LC1 = 1
const val LC2 = 2
const val LC3 = 3
const val LC4 = 4
const val LC5 = 5
const val LC6 = 6
const val LC7 = 7

// PID_LIST_END. XTypes 1.3 §7.4.1.2 Table 31.
const val PID_LIST_END_ID = 0x3F02

// The PID that introduces the twelve-octet parameter header. §7.4.1.2.
const val PID_EXTENDED_ID = 0x3F01

// The highest member id a short parameter header carries, inclusive.
const val PID_SHORT_MAX_ID = 0x3F00

// Bit 14 of a short parameter id. XTypes 1.3 §7.4.1.2.
private const val PID_FLAG_MUST_UNDERSTAND = 0x4000

// Bit 30 of the eMemberHeader inside an extended parameter header — not bit 31,
// which is the implementation-extension flag.
private const val EM_FLAG_MUST_UNDERSTAND = 0x40000000L

// The octets an argument of `bits` width may carry.
//
// Kotlin's integers are signed and wider than every wire width below 64, so a
// value that fits neither the signed nor the unsigned range of the width cannot
// be a pattern of that width at all. Truncating it is the defect this refuses:
// `putU16(70000)` wrote `7011` — 70000 and 4464 are different numbers and the
// frame was the right length either way, so nothing downstream could tell.
private fun pattern(v: Long, bits: Int, call: String): Long {
    if (bits == 64) return v // every Long is a 64-bit pattern
    val unsignedMax = (1L shl bits) - 1L
    val signedMin = -(1L shl (bits - 1))
    require(v in signedMin..unsignedMax) {
        "$call: $v is not a $bits-bit value ($signedMin..$unsignedMax)"
    }
    return v and unsignedMax
}

class Writer(private val endian: Endian, private val encoding: Encoding) {
    private val buf = ArrayList<Byte>()

    // The alignment cap this encoding imposes. XTypes 1.3 §7.4.1.
    val maxAlign: Int get() = encoding.maxAlign

    // The position alignment is measured from. Zero for the stream, and moved to
    // the start of a PL_CDR1 parameter's value for the length of that parameter
    // — §7.4.1.2 aligns a parameter's value against its own block, not against
    // the stream. Without it a `double` in a parameter takes four octets of
    // padding and the header declares twelve where Cyclone, RTI and Fast DDS all
    // declare eight.
    private var origin = 0

    private fun alignTo(a: Int) {
        val cap = if (a > maxAlign) maxAlign else a
        while ((buf.size - origin) % cap != 0) buf.add(0)
    }

    private fun putLE(a: Int, le: ByteArray) {
        alignTo(a)
        if (endian == Endian.BIG) for (i in le.indices.reversed()) buf.add(le[i])
        else for (b in le) buf.add(b)
    }

    fun putU8(v: Int) { buf.add(pattern(v.toLong(), 8, "putU8").toByte()) }

    // A `boolean` is the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it: 0 or
    // 1, never any other pattern.
    fun putBool(v: Boolean) { buf.add(if (v) 1 else 0) }

    fun putU16(v: Int) {
        val p = pattern(v.toLong(), 16, "putU16")
        putLE(2, byteArrayOf(p.toByte(), (p ushr 8).toByte()))
    }
    fun putU32(v: Long) {
        val p = pattern(v, 32, "putU32")
        putLE(4, byteArrayOf(p.toByte(), (p ushr 8).toByte(), (p ushr 16).toByte(), (p ushr 24).toByte()))
    }

    // A `long long` at its natural eight-octet alignment, which maxAlign then
    // caps: eight under XCDR1 and four under XCDR2. XTypes 1.3 §7.4.1.
    fun putU64(v: Long) = putLE(8, ByteArray(8) { i -> (v ushr (8 * i)).toByte() })
    fun putF32(v: Float) = putU32(java.lang.Float.floatToRawIntBits(v).toLong() and 0xFFFFFFFFL)

    // `double` is one of the fifteen primitives of XTypes 1.3 §7.2.2.2 and the
    // floating type most DDS topics use; it goes out as its IEEE-754 bit pattern
    // at the same alignment as putU64.
    fun putF64(v: Double) = putU64(java.lang.Double.doubleToRawLongBits(v))
    fun putBytes(b: ByteArray) { for (x in b) buf.add(x) }
    // Wire rule (docs/specs/zerodds-xcdr2-kotlin-1.0.md §"string"): uint32
    // (UTF-8 byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is the
    // BYTE count of the UTF-8 encoding, not the Kotlin char count — multibyte
    // characters occupy more than one byte. Byte-identical to the C/Rust core.
    fun putString(s: String) {
        val raw = s.toByteArray(Charsets.UTF_8)
        putU32((raw.size + 1).toLong())
        putBytes(raw)
        putU8(0)
    }
    fun putSeqU8(b: ByteArray) { putU32(b.size.toLong()); putBytes(b) }

    // An IDL `char`: the one octet XTypes 1.3 §7.4.1.1.1 Table 30 gives it.
    //
    // A code point above U+00FF is refused and not truncated. `š` is U+0161 and
    // its low octet is 0x61, which is `a`: a truncating writer produces a frame
    // of the right length holding a different, plausible character, and nothing
    // downstream can tell. The Rust core refuses `primitive-char-above-latin1`
    // in the same words.
    fun putChar(codePoint: Int) {
        require(codePoint in 0..0xFF) {
            "expected a character below U+0100, found U+%04X".format(codePoint)
        }
        buf.add(codePoint.toByte())
    }

    // An IDL `wchar`: one two-octet code unit, 2-aligned, in the stream's byte
    // order.
    //
    // The four policy axes a wide character carries are answered here and not by
    // the caller: the unit is two octets wide, it goes out in the stream's byte
    // order, there is no GIOP length octet before it, and no byte-order mark.
    //
    // A code point above U+FFFF is refused. The refusal is width-independent — a
    // four-octet form would zero-extend a code *unit*, not a code point — which
    // is how the Rust core refuses `primitive-wchar-above-bmp`.
    fun putWchar(codePoint: Int) {
        require(codePoint in 0..0xFFFF) {
            "expected a BMP character, found U+%04X".format(codePoint)
        }
        putU16(codePoint)
    }

    // An IDL `wstring` from its UTF-16 code units.
    //
    // The length word counts **octets** and not code units, there is no
    // terminator, there is no byte-order mark, and each unit goes out two octets
    // wide in the stream's own byte order. Those are the four axes, and they are
    // the binding's answer — the caller hands over units and nothing else.
    fun putWstring(units: IntArray) {
        putU32((units.size * 2).toLong())
        for (unit in units) {
            require(unit in 0..0xFFFF) { "a wstring code unit is two octets, found $unit" }
            putU16(unit)
        }
    }

    // A `bitset` as its anchor word: the holder the declared bits pack into,
    // written at the anchor's own width and therefore at the anchor's own
    // alignment — eight under XCDR1 and four under XCDR2 for an eight-octet
    // anchor. XTypes 1.3 §7.3.1.4 and §7.4.1.
    //
    // `bits` is the packed holder and `anchor` its width in octets, both
    // properties of the declared type. What this call decides is the pair the
    // type does not: which write the holder goes out through, and what that
    // write aligns to.
    fun putBitset(bits: Long, anchor: Int) {
        when (anchor) {
            1 -> putU8((bits and 0xFF).toInt())
            2 -> putU16((bits and 0xFFFF).toInt())
            4 -> putU32(bits and 0xFFFFFFFFL)
            8 -> putU64(bits)
            else -> throw IllegalArgumentException(
                "a bitset anchor is one, two, four or eight octets, not $anchor"
            )
        }
    }

    // An IDL `fixed<digits,scale>` as packed BCD.
    // CDR (OMG CORBA 3.3) §9.3.2.8 / IDL 4.2 §7.4.1.4.5.
    fun putFixed(value: String, digits: Int, scale: Int) = putBytes(fixedOctets(value, digits, scale))

    // --- back-patched framing --------------------------------------------
    //
    // A DHEADER, a NEXTINT and a PL_CDR1 parameter length are all the same
    // shape: a slot reserved before the body and filled in once the body's
    // length is known. These are the only writes into an already-written
    // position, and they honour the wire byte order like every other write.

    private fun patchU32(at: Int, v: Long) {
        val le = byteArrayOf(v.toByte(), (v ushr 8).toByte(), (v ushr 16).toByte(), (v ushr 24).toByte())
        for (i in 0 until 4) buf[at + i] = if (endian == Endian.BIG) le[3 - i] else le[i]
    }

    private fun patchU16(at: Int, v: Int) {
        val le = byteArrayOf(v.toByte(), (v ushr 8).toByte())
        for (i in 0 until 2) buf[at + i] = if (endian == Endian.BIG) le[1 - i] else le[i]
    }

    // Reserves a 4-aligned DHEADER slot and returns the position the body starts
    // at, to hand back to dheaderEnd. XTypes 1.3 §7.4.3.5.
    fun dheaderBegin(): Int {
        putU32(0)
        return buf.size
    }

    // Back-patches the DHEADER with the byte length written since bodyStart. The
    // DHEADER itself is not counted — §7.4.3.5 measures the content that follows
    // it.
    fun dheaderEnd(bodyStart: Int) = patchU32(bodyStart - 4, (buf.size - bodyStart).toLong())

    // Writes the EMHEADER for one member of a @mutable aggregate and returns the
    // body-start position. XTypes 1.3 §7.4.3.4.2:
    //
    //     EMHEADER = (must_understand << 31) | (length_code << 28) | member_id
    //
    // with a 28-bit member id. For LC4 a NEXTINT slot is reserved after it and
    // emheaderEnd fills it in; for LC0..LC3 the code alone gives the length and
    // nothing follows; for LC5..LC7 the body's own leading four octets are the
    // length, so no separate NEXTINT is written and emheaderEnd has nothing to
    // patch.
    fun emheaderBegin(id: Long, mustUnderstand: Boolean, lengthCode: Int): Int {
        require(id in 0..0x0FFFFFFFL) { "member id $id does not fit the 28 bits of an EMHEADER" }
        require(lengthCode in LC0..LC7) { "length code $lengthCode is not one of LC0..LC7" }
        var header = (lengthCode.toLong() shl 28) or id
        if (mustUnderstand) header = header or 0x80000000L
        putU32(header)
        if (lengthCode == LC4) putU32(0)
        return buf.size
    }

    // Closes the member emheaderBegin opened.
    //
    // It takes the length code back rather than remembering it, so a caller that
    // paired an LC4 begin with a differently-coded end gets a refusal instead of
    // a NEXTINT patched over the first four octets of a member body.
    fun emheaderEnd(bodyStart: Int, lengthCode: Int) {
        val written = buf.size - bodyStart
        when (lengthCode) {
            LC4 -> patchU32(bodyStart - 4, written.toLong())
            // 1, 2, 4, 8 — the code *is* the length, so a body of any other size
            // means the plan and the member disagree. Silence here would put a
            // length on the wire that no octet count backs.
            LC0, LC1, LC2, LC3 -> {
                val want = 1 shl lengthCode
                require(written == want) { "LC$lengthCode frames $want octets and the member wrote $written" }
            }
            LC5, LC6, LC7 -> require(written >= 4) {
                "LC$lengthCode takes its length from the body's first four octets and the member wrote $written"
            }
        }
    }

    // An open PL_CDR1 parameter header, to be handed back to pidEnd.
    class Pid(val at: Int, val extended: Boolean, val savedOrigin: Int)

    // Writes a PL_CDR1 short parameter header. XTypes 1.3 §7.4.1.2: at a
    // four-octet boundary an `unsigned short` parameter id, then an `unsigned
    // short` byte length of the value that follows. Both go out in the stream's
    // byte order.
    //
    // The must-understand flag is bit 14 of the id field. Bit 15 is the
    // implementation-extension flag and is not set here.
    //
    // The parameter's value aligns against the block and not against the stream,
    // so the alignment origin moves for the length of the parameter and pidEnd
    // restores it.
    fun pidBegin(pid: Int, mustUnderstand: Boolean): Pid {
        require(pid in 0..PID_SHORT_MAX_ID) { "member id $pid needs the extended parameter header" }
        alignTo(4)
        var id = pid
        if (mustUnderstand) id = id or PID_FLAG_MUST_UNDERSTAND
        putLE(2, byteArrayOf(id.toByte(), (id ushr 8).toByte()))
        val at = buf.size
        buf.add(0); buf.add(0)
        val saved = origin
        origin = buf.size
        return Pid(at, false, saved)
    }

    // Writes the twelve-octet parameter header XTypes 1.3 §7.4.1.2 requires for
    // a member id above 0x3F00 or a value longer than 65535 octets:
    //
    //     [PID_EXTENDED | mu : u16][8 : u16][eMemberHeader : u32][length : u32]
    //
    // The must-understand flag on the marker is not the member's — it is on the
    // marker under this framing whatever the member says, and the member's own
    // flag is bit 30 of the eMemberHeader. Reading it off the marker instead
    // makes every parameter must-understand, which is what Cyclone's wire looks
    // like if you do.
    fun pidExtendedBegin(memberId: Long, mustUnderstand: Boolean): Pid {
        require(memberId in 0..0x0FFFFFFFL) {
            "member id $memberId does not fit the 28 bits of an eMemberHeader"
        }
        alignTo(4)
        val marker = PID_EXTENDED_ID or PID_FLAG_MUST_UNDERSTAND
        putLE(2, byteArrayOf(marker.toByte(), (marker ushr 8).toByte()))
        putLE(2, byteArrayOf(8, 0))
        var header = memberId
        if (mustUnderstand) header = header or EM_FLAG_MUST_UNDERSTAND
        putLE(4, byteArrayOf(header.toByte(), (header ushr 8).toByte(), (header ushr 16).toByte(), (header ushr 24).toByte()))
        val at = buf.size
        buf.add(0); buf.add(0); buf.add(0); buf.add(0)
        val saved = origin
        origin = buf.size
        return Pid(at, true, saved)
    }

    // Closes the parameter pidBegin or pidExtendedBegin opened.
    //
    // The declared length counts the value's octets **and** the padding that
    // brings the next header back to a four-octet boundary: a reader skips
    // `length` from the end of the header and has to land on the next parameter.
    // The padding itself is written after the length is patched, so the two
    // cannot disagree.
    fun pidEnd(header: Pid) {
        val width = if (header.extended) 4 else 2
        var length = buf.size - header.at - width
        // The parameter's own origin is still in force here, so the round-up is
        // measured from the block that the length describes.
        val past = (buf.size - header.savedOrigin) % 4
        if (past != 0) length += 4 - past
        if (header.extended) {
            patchU32(header.at, length.toLong())
        } else {
            require(length <= 0xFFFF) { "a short parameter header describes at most 65535 octets, not $length" }
            patchU16(header.at, length)
        }
        origin = header.savedOrigin
        alignTo(4)
    }

    // The sentinel that closes a PL_CDR1 parameter list: the id PID_LIST_END
    // with a zero length, four-aligned. XTypes 1.3 §7.4.1.2.
    //
    // The must-understand flag is set on the sentinel — 0x7F02 on the wire,
    // which is what Cyclone and RTI write. Fast DDS writes the bare 0x3F02; the
    // flag is not the sentinel's meaning either way, and this is the majority
    // form the Rust column records.
    fun pidListEnd() {
        alignTo(4)
        val id = PID_LIST_END_ID or PID_FLAG_MUST_UNDERSTAND
        putLE(2, byteArrayOf(id.toByte(), (id ushr 8).toByte()))
        putLE(2, byteArrayOf(0, 0))
    }

    fun bytes(): ByteArray = buf.toByteArray()
}

// The packing putFixed writes, apart so it can be tested at the edges of its
// range without a Writer.
//
// `value` is the decimal as the sample wrote it — an optional sign, an integer
// part and an optional fraction. Everything after that is this function's:
//
//   - the integer part is left-padded and the fraction right-padded to exactly
//     `digits` decimal digits, with the point implied `scale` from the right;
//     the scale itself is not on the wire,
//   - a leading zero nibble is prepended when `digits + 1` is odd, so digits and
//     sign pair up,
//   - two nibbles to the octet, most significant digit first, high nibble first,
//   - the last nibble is the sign: 0xD negative, 0xC otherwise. A negative zero
//     goes out positive, because there is one zero to write,
//   - alignment is one, and no multi-octet integer is written, so the byte order
//     never enters.
//
// The octet count is therefore ceil((digits + 1) / 2), whatever the value.
fun fixedOctets(value: String, digits: Int, scale: Int): ByteArray {
    require(digits in 1..31) { "fixed<$digits,$scale>: a digit count is 1..31" }
    require(scale in 0..digits) { "fixed<$digits,$scale>: a scale is 0..digits" }
    var text = value
    var negative = false
    if (text.isNotEmpty() && (text[0] == '+' || text[0] == '-')) {
        negative = text[0] == '-'
        text = text.substring(1)
    }
    val dot = text.indexOf('.')
    var integral = if (dot >= 0) text.substring(0, dot) else text
    var fraction = if (dot >= 0) text.substring(dot + 1) else ""
    require(integral.isNotEmpty() || fraction.isNotEmpty()) {
        "fixed<$digits,$scale>: `$value` has no digits"
    }
    for (part in listOf(integral, fraction)) {
        for (ch in part) require(ch in '0'..'9') { "fixed<$digits,$scale>: `$value` is not a decimal" }
    }
    // Leading zeros carry no value, so they are dropped before the width is
    // checked: `000000.001` is one significant integer digit, not six.
    integral = integral.trimStart('0')
    require(integral.length <= digits - scale) {
        "fixed<$digits,$scale>: `$value` needs ${integral.length} integer digits and the type has ${digits - scale}"
    }
    // A trailing zero in the fraction is padding either way, so it is dropped
    // before the width is checked and re-added below.
    while (fraction.length > scale && fraction.endsWith("0")) fraction = fraction.dropLast(1)
    require(fraction.length <= scale) {
        "fixed<$digits,$scale>: `$value` needs ${fraction.length} fraction digits and the type has $scale"
    }
    val nibbles = ArrayList<Int>(digits + 2)
    if ((digits + 1) % 2 == 1) nibbles.add(0)
    repeat(digits - scale - integral.length) { nibbles.add(0) }
    for (ch in integral) nibbles.add(ch - '0')
    for (ch in fraction) nibbles.add(ch - '0')
    repeat(scale - fraction.length) { nibbles.add(0) }
    nibbles.add(if (negative && nibbles.any { it != 0 }) 0x0D else 0x0C)
    val out = ByteArray(nibbles.size / 2)
    for (i in out.indices) out[i] = ((nibbles[2 * i] shl 4) or nibbles[2 * i + 1]).toByte()
    return out
}

class Reader(private val buf: ByteArray, private val endian: Endian, private val encoding: Encoding) {
    var pos = 0

    // The same cap the writer used, or the fields land elsewhere.
    val maxAlign: Int get() = encoding.maxAlign

    // Mirrors the writer's: a PL_CDR1 parameter's value aligns against its own
    // block.
    private var origin = 0

    private fun getLE(a: Int, n: Int): ByteArray {
        val cap = if (a > maxAlign) maxAlign else a
        while ((pos - origin) % cap != 0) pos++
        val out = buf.copyOfRange(pos, pos + n)
        pos += n
        if (endian == Endian.BIG) out.reverse()
        return out
    }

    fun getU32(): Long {
        val le = getLE(4, 4)
        return (le[0].toLong() and 0xFF) or ((le[1].toLong() and 0xFF) shl 8) or
            ((le[2].toLong() and 0xFF) shl 16) or ((le[3].toLong() and 0xFF) shl 24)
    }

    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
    fun getU8(): Int = (buf[pos].toInt() and 0xFF).also { pos++ }
    fun getU16(): Int {
        val le = getLE(2, 2)
        return (le[0].toInt() and 0xFF) or ((le[1].toInt() and 0xFF) shl 8)
    }
    // A `long long` at its natural eight-octet alignment, which maxAlign then
    // caps: eight under XCDR1 and four under XCDR2.
    //
    // It passed 4 here while the writer passed 8, which under XCDR1 read the
    // four octets of padding as the value's low half and returned a different
    // number with no error anywhere.
    fun getU64(): Long {
        val le = getLE(8, 8)
        var v = 0L
        for (i in 0 until 8) v = v or ((le[i].toLong() and 0xFF) shl (8 * i))
        return v
    }
    fun getF32(): Float = java.lang.Float.intBitsToFloat(getU32().toInt())
    fun getF64(): Double = java.lang.Double.longBitsToDouble(getU64())
    fun getBool(): Boolean = getU8() != 0

    // The one octet an IDL `char` occupies.
    fun getChar(): Int = getU8()

    // One two-octet `wchar` code unit.
    fun getWchar(): Int = getU16()

    // A `wstring` back into its UTF-16 code units. The length word counts
    // octets, so the unit count is half of it.
    fun getWstring(): IntArray {
        val octets = getU32().toInt()
        return IntArray(octets / 2) { getU16() }
    }

    // A `bitset` anchor of the given width back into its holder word.
    fun getBitset(anchor: Int): Long = when (anchor) {
        1 -> getU8().toLong()
        2 -> getU16().toLong()
        4 -> getU32()
        8 -> getU64()
        else -> throw IllegalArgumentException(
            "a bitset anchor is one, two, four or eight octets, not $anchor"
        )
    }

    // A `fixed<digits,scale>` back into the decimal it was written from: a
    // leading `-` when the sign nibble is 0xD, `digits - scale` integer digits,
    // and a `.` with `scale` fraction digits where the scale is not zero.
    //
    // The scale is not on the wire, so it is an argument here for the same
    // reason it is one in putFixed.
    fun getFixed(digits: Int, scale: Int): String {
        val octets = (digits + 2) / 2
        val nibbles = ArrayList<Int>(octets * 2)
        repeat(octets) {
            val b = getU8()
            nibbles.add(b shr 4); nibbles.add(b and 0x0F)
        }
        val sign = nibbles[nibbles.size - 1]
        val body = nibbles.subList(nibbles.size - 1 - digits, nibbles.size - 1)
        val out = StringBuilder()
        if (sign == 0x0D) out.append('-')
        for ((i, n) in body.withIndex()) {
            if (scale != 0 && i == digits - scale) out.append('.')
            out.append(('0' + n))
        }
        return out.toString()
    }

    // Reads a DHEADER and returns the announced body byte length.
    fun dheaderRead(): Long = getU32()

    // A member header: the member id, the must-understand flag, the length code,
    // and the member body's extent in octets measured from the position this
    // call leaves the cursor at.
    //
    // LC0..LC3 take the extent from the code. LC4 consumes the NEXTINT that
    // follows the header, and the extent is that word. LC5..LC7 do not consume
    // anything: the next four octets are the member's own leading word and
    // belong to the body, so they are peeked and the extent is 4 + the word for
    // LC5, 4 + 4×word for LC6 and 4 + 8×word for LC7 — the last two count
    // elements, not octets.
    class Member(val id: Long, val mustUnderstand: Boolean, val lengthCode: Int, val extent: Long)

    fun emheaderRead(): Member {
        val header = getU32()
        val id = header and 0x0FFFFFFFL
        val mustUnderstand = (header and 0x80000000L) != 0L
        val lengthCode = ((header shr 28) and 0x7L).toInt()
        val extent = when (lengthCode) {
            LC0, LC1, LC2, LC3 -> (1L shl lengthCode)
            LC4 -> getU32()
            else -> {
                val at = pos
                val word = getU32()
                pos = at
                when (lengthCode) {
                    LC5 -> 4 + word
                    LC6 -> 4 + 4 * word
                    else -> 4 + 8 * word
                }
            }
        }
        return Member(id, mustUnderstand, lengthCode, extent)
    }

    // A PL_CDR1 parameter header in either form: the member id with its flags
    // stripped, the member's must-understand flag, the declared byte length, and
    // whether the list ends here.
    //
    // The extended form is recognised by the marker PID_EXTENDED, and the
    // must-understand answer then comes from bit 30 of the eMemberHeader and
    // never from the marker — the marker carries the flag under this framing
    // whatever the member says.
    //
    // The value's alignment origin is moved to the position this call leaves,
    // and pidSkip restores it.
    class Parameter(val id: Long, val mustUnderstand: Boolean, val length: Long, val listEnd: Boolean)

    fun pidRead(): Parameter {
        val raw = getU16()
        val short = raw and 0x3FFF
        if (short == PID_LIST_END_ID) {
            getU16()
            return Parameter(0, false, 0, true)
        }
        if (short == PID_EXTENDED_ID) {
            getU16() // the slength, always 8
            val header = getU32()
            val length = getU32()
            origin = pos
            return Parameter(header and 0x0FFFFFFFL, (header and EM_FLAG_MUST_UNDERSTAND) != 0L, length, false)
        }
        val length = getU16().toLong()
        origin = pos
        return Parameter(short.toLong(), (raw and PID_FLAG_MUST_UNDERSTAND) != 0, length, false)
    }

    // Moves the cursor past a parameter value of `length` octets from
    // `bodyStart` and restores the stream's alignment origin.
    fun pidSkip(bodyStart: Int, length: Long) {
        pos = bodyStart + length.toInt()
        origin = 0
        while (pos % 4 != 0) pos++
    }
    // Inverse of putString: read n bytes (n incl. the NUL), decode the first
    // n-1 as UTF-8.
    fun getString(): String {
        val n = getU32().toInt()
        val s = String(buf, pos, n - 1, Charsets.UTF_8)
        pos += n
        return s
    }
    fun getSeqU8(): ByteArray {
        val n = getU32().toInt()
        val b = buf.copyOfRange(pos, pos + n)
        pos += n
        return b
    }
}

// --- XRCE framing ---

const val XRCE_SESSION_NOKEY = 0x80
const val XRCE_STREAM_BEST_EFFORT = 0x01

// E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits 1..3 —
// DDS-XRCE 1.0 §8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would additionally
// require a SampleInfo in the body, which this profile does not carry.
const val XRCE_WRITE_FLAGS = 0x01

/** Wire size of the BaseObjectRequest that precedes the sample (§7.7.8). */
const val XRCE_BASE_SIZE = 4

/** `OBJK_DATAWRITER` — the ObjectKind of a DataWriter (§7.2.2). */
const val OBJK_DATAWRITER = 0x05

/** An ObjectId (§7.2.1) from a 12-bit raw id and a 4-bit ObjectKind. */
fun xrceObjectId(rawId: Int, kind: Int): Int = ((rawId and 0x0FFF) shl 4) or (kind and 0x0F)

// The submessage body is the BaseObjectRequest (§7.7.8: request_id then
// object_id, each two octets most significant first) followed by the sample.
// `objectId` names the target DataWriter — without it no XRCE agent can route
// the sample. The request_id is the message's own `seq`, which §7.7.8 only
// requires to be unique per stream.
//
// The 16-bit submessage_length field cannot describe a body larger than
// 0xFFFF; refuse rather than truncate the length while copying the full payload
// (the C SDK rejects the same way).
fun xrceWriteFrame(session: Int, stream: Int, seq: Int, objectId: Int, sample: ByteArray): ByteArray {
    val bodyLen = XRCE_BASE_SIZE + sample.size
    require(bodyLen <= 0xFFFF) { "sample exceeds 16-bit submessage_length" }
    val out = ByteArray(8 + bodyLen)
    out[0] = session.toByte(); out[1] = stream.toByte()
    out[2] = seq.toByte(); out[3] = (seq ushr 8).toByte()
    out[4] = 0x07; out[5] = XRCE_WRITE_FLAGS.toByte()
    out[6] = bodyLen.toByte(); out[7] = (bodyLen ushr 8).toByte()
    out[8] = (seq ushr 8).toByte(); out[9] = seq.toByte()
    out[10] = (objectId ushr 8).toByte(); out[11] = objectId.toByte()
    System.arraycopy(sample, 0, out, 8 + XRCE_BASE_SIZE, sample.size)
    return out
}

// Direction (DDS-XRCE spec §8.3.5): WRITE_DATA (0x07) is client->agent — what
// this endpoint SENDS (xrceWriteFrame); DATA (0x09) is agent->client — what it
// RECEIVES from the hub. The reader accepts both (parity with the C/Python
// SDKs): 0x09 for real hub traffic, 0x07 for loopback. It returns null — never
// throws, never a bogus sample — on a short header, an unknown submessage id, or
// a declared length that runs past the datagram (truncated / wrong length); the
// body is bounded by the declared length so trailing bytes never leak in.
fun xrceReadFrame(frame: ByteArray): ByteArray? {
    if (frame.size < 8 + XRCE_BASE_SIZE) return null
    val id = frame[4].toInt() and 0xFF
    if (id != 0x09 && id != 0x07) return null
    val smLen = (frame[6].toInt() and 0xFF) or ((frame[7].toInt() and 0xFF) shl 8)
    if (smLen < XRCE_BASE_SIZE || 8 + smLen > frame.size) return null
    return frame.copyOfRange(8 + XRCE_BASE_SIZE, 8 + smLen)
}

/** `(request_id, object_id)` of a data frame (§7.7.8), or null under the same
 *  rules as [xrceReadFrame]. */
fun xrceFrameBase(frame: ByteArray): Pair<Int, Int>? {
    if (xrceReadFrame(frame) == null) return null
    return Pair(
        ((frame[8].toInt() and 0xFF) shl 8) or (frame[9].toInt() and 0xFF),
        ((frame[10].toInt() and 0xFF) shl 8) or (frame[11].toInt() and 0xFF),
    )
}

// --- transport ---

interface Transport {
    fun deliver(frame: ByteArray): Boolean
    // Returns one frame, or null when nothing is ready.
    fun receive(): ByteArray?
}

// --- sync client ---

class Client(
    private val transport: Transport,
    /** ObjectId of the target DataWriter (§7.7.8 BaseObjectRequest). */
    private val objectId: Int,
    private val session: Int = XRCE_SESSION_NOKEY,
    private val stream: Int = XRCE_STREAM_BEST_EFFORT,
) {
    private var seq = 1
    fun write(sample: ByteArray): Boolean {
        val frame = xrceWriteFrame(session, stream, seq, objectId, sample)
        seq++
        return transport.deliver(frame)
    }
    fun poll(): ByteArray? = transport.receive()?.let { xrceReadFrame(it) }
}

// --- async reader: a background thread pushes decoded bodies onto a channel ---

class AsyncReader(private val transport: Transport) {
    val samples = LinkedBlockingQueue<ByteArray>()
    @Volatile private var running = true

    private val thread = Thread {
        while (running) {
            val frame = transport.receive()
            if (frame == null) { Thread.sleep(1); continue }
            xrceReadFrame(frame)?.let { samples.put(it) }
        }
    }.apply { isDaemon = true; start() }

    fun close() { running = false }
}

// A thread-safe in-memory transport used by the tests + example.
class MemTransport : Transport {
    private val q = ConcurrentLinkedQueue<ByteArray>()
    override fun deliver(frame: ByteArray): Boolean { q.add(frame.copyOf()); return true }
    override fun receive(): ByteArray? = q.poll()
}

// A real UDP transport: one DatagramSocket carries both directions, so the peer
// replies to the app's source port. `receive()` polls with a short SO_TIMEOUT
// and returns null on expiry (never blocks the AsyncReader drain forever). This
// is the drop-in the QUICKSTART mentions — the sync Client and the AsyncReader
// run unchanged over a live datagram link instead of MemTransport.
class UdpTransport(
    private val host: java.net.InetAddress,
    private val port: Int,
    pollMs: Int = 100,
) : Transport {
    private val sock = java.net.DatagramSocket().apply { soTimeout = pollMs }

    override fun deliver(frame: ByteArray): Boolean {
        sock.send(java.net.DatagramPacket(frame, frame.size, host, port))
        return true
    }

    override fun receive(): ByteArray? {
        val buf = ByteArray(4096)
        val pkt = java.net.DatagramPacket(buf, buf.size)
        return try {
            sock.receive(pkt)
            pkt.data.copyOf(pkt.length)
        } catch (e: java.net.SocketTimeoutException) {
            null
        }
    }

    fun close() = sock.close()
}
