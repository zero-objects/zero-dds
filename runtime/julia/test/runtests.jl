# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# The runtime's own smoke tests for the three axes `Dialect::Hdds`
# selects. Expected octets from the recorded HDDS columns:
# tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin
# (`04 00 00 00 61 62 63 00` for "abc"),
# tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin
# (`01 00 00 00 02 00 03 00 00 00 04 00 77`) and
# tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin
# (the first Cell of `one`, `06 00 00 00 79 29 ed ff fe ff`).

using Test

include(joinpath(@__DIR__, "..", "zerodds_wire.jl"))
using .ZeroddsWire

const FRAMING = ZdPlCdr1Framing(
    pid_flags = "sentinel-must-understand",
    highest_short_id = UInt32(0x3EFF),
    pad_declared_length = false,
    pad_declared_optional_length = false,
    member_must_understand = true,
    pad_after_value = true,
)

@testset "the three HDDS axes" begin
    @testset "a UTF-8 wstring counts its octets and a NUL" begin
        w = ZdWriter(ZD_LITTLE, 4, FRAMING, false)
        write_wstring_utf8_octets(w, "abc", "octets/terminated/2")
        @test zd_writer_bytes(w) == UInt8[0x04, 0, 0, 0, 0x61, 0x62, 0x63, 0x00]
        r = ZdReader(zd_writer_bytes(w), ZD_LITTLE, 4, FRAMING)
        @test read_wstring_utf8_octets(r, "octets/terminated/2") == "abc"
    end

    @testset "a multibyte UTF-8 wstring counts its octets" begin
        w = ZdWriter(ZD_LITTLE, 4, FRAMING, false)
        write_wstring_utf8_octets(w, "äöü ሴ Z", "octets/terminated/2")
        @test zd_writer_bytes(w) == UInt8[
            0x0d, 0, 0, 0, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc, 0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0x00,
        ]
        r = ZdReader(
            UInt8[0x0d, 0, 0, 0, 0xc3, 0xa4, 0xc3, 0xb6, 0xc3, 0xbc, 0x20, 0xe1, 0x88, 0xb4, 0x20, 0x5a, 0x00],
            ZD_LITTLE,
            4,
            FRAMING,
        )
        @test read_wstring_utf8_octets(r, "octets/terminated/2") == "äöü ሴ Z"
    end

    @testset "a bounded UTF-8 wstring bounds its UTF-16 units" begin
        w = ZdWriter(ZD_LITTLE, 4, FRAMING, false)
        write_wstring_utf8_octets_bounded(w, "ሴäZ", 3, "octets/terminated/2")
        @test zd_writer_bytes(w) == UInt8[0x07, 0, 0, 0, 0xe1, 0x88, 0xb4, 0xc3, 0xa4, 0x5a, 0x00]
        @test_throws ZdOutOfRange write_wstring_utf8_octets_bounded(
            ZdWriter(ZD_LITTLE, 4, FRAMING, false),
            "ሴäZ",
            2,
            "octets/terminated/2",
        )
    end

    @testset "a packed element counts alignment from its own start" begin
        w = ZdWriter(ZD_LITTLE, 4, FRAMING, false)
        saved = begin_packed_element(w)
        write_i32(w, 1)
        write_i16(w, 2)
        finish_packed_element(w, saved)
        saved = begin_packed_element(w)
        write_i32(w, 3)
        write_i16(w, 4)
        finish_packed_element(w, saved)
        write_u8(w, 119)
        recorded = UInt8[1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 119]
        @test zd_writer_bytes(w) == recorded
        r = ZdReader(recorded, ZD_LITTLE, 4, FRAMING)
        saved = reader_begin_packed_element(r)
        @test read_i32(r) == 1
        @test read_i16(r) == 2
        reader_finish_packed_element(r, saved)
        saved = reader_begin_packed_element(r)
        @test read_i32(r) == 3
        @test read_i16(r) == 4
        reader_finish_packed_element(r, saved)
        @test read_u8(r) == 119
    end

    @testset "a sequence element length frames the element it precedes" begin
        w = ZdWriter(ZD_LITTLE, 4, FRAMING, false)
        header = write_dheader(w)
        write_i32(w, -1234567)
        write_i16(w, -2)
        finish_dheader(w, header)
        got = zd_writer_bytes(w)
        @test got == UInt8[6, 0, 0, 0, 0x79, 0x29, 0xed, 0xff, 0xfe, 0xff]
        r = ZdReader(got, ZD_LITTLE, 4, FRAMING)
        declared = Int64(read_u32(r))
        end_at = reader_position(r) + declared
        @test read_i32(r) == -1234567
        @test read_i16(r) == -2
        @test reader_position(r) == end_at
    end
end

# The five operations the generated code reaches only in a dialect or with a
# member no corpus case carries: write_pid_extended, write_union_overlay,
# reader_clear_discard, reader_save and reader_restore
# (internal/release/OPEN-ITEMS.md). Every runtime checks the same four
# vectors, each against an oracle outside this code.
const SPECIFICATION = ZdPlCdr1Framing(
    pid_flags = "sentinel-must-understand",
    highest_short_id = UInt32(0x3F00),
    pad_declared_length = false,
    pad_declared_optional_length = false,
    member_must_understand = true,
    pad_after_value = true,
)

@testset "the operations no corpus case reaches" begin
    @testset "write_pid_extended writes Cyclone's recorded parameter" begin
        # The first parameter of `annotation-autoid-sequential` as Cyclone
        # DDS recorded it, XCDR1 little endian
        # (tests/interop-corpus/expected-bytes.tsv).
        w = ZdWriter(ZD_LITTLE, 8, SPECIFICATION, false)
        header = write_pid_extended(w, 0, ZD_PARAMETER_MEMBER, false)
        write_i32(w, -1234567)
        finish_pid(w, header)
        @test zd_writer_bytes(w) ==
              UInt8[0x01, 0x7f, 0x08, 0x00, 0, 0, 0, 0, 0x04, 0, 0, 0, 0x79, 0x29, 0xed, 0xff]
    end

    @testset "write_union_overlay writes the storage at the branch's width" begin
        # A `short` read through a four-octet and a one-octet `default:`
        # branch: Cyclone DDS 11.0.1 writes the union's storage, the member's
        # octets with zeros above (`combined-everything`,
        # crates/wire-codec/src/encode.rs `write_selected_branch`).
        w = ZdWriter(ZD_LITTLE, 4, SPECIFICATION, false)
        pattern = bit_pattern_int(-2, 2)
        write_union_overlay(w, pattern, 4)
        write_union_overlay(w, pattern, 1)
        @test zd_writer_bytes(w) == UInt8[0xfe, 0xff, 0, 0, 0xfe]
    end

    @testset "reader_clear_discard takes back a discard and nothing else" begin
        # XTypes 1.3 §7.2.2.4.4.4.9: a member's explicit @default comes
        # before Table 9's value, so the call site asks for DISCARD and takes
        # it back. Any other refusal is not a narrowing and stays.
        discarded = ZdReader(UInt8[1, 2, 3, 4], ZD_LITTLE, 4, SPECIFICATION)
        reader_refuse(discarded, ZD_STATUS_DISCARDED)
        @test reader_clear_discard(discarded)
        @test reader_status(discarded) === ZD_STATUS_OK
        refused = ZdReader(UInt8[1, 2, 3, 4], ZD_LITTLE, 4, SPECIFICATION)
        reader_refuse(refused, ZD_STATUS_OUT_OF_RANGE)
        @test !reader_clear_discard(refused)
        @test reader_status(refused) === ZD_STATUS_OUT_OF_RANGE
    end

    @testset "reader_save and reader_restore count a delimited map and read it again" begin
        # XTypes 1.3 §7.4.3.5.3 rule (15): a delimited map<long, long> as its
        # DHEADER and the pairs, without a count. The reader counts the
        # pairs, goes back and reads them again; a refusal made while
        # counting goes back with the cursor.
        r = ZdReader(UInt8[0x10, 0, 0, 0, 1, 0, 0, 0, 0x0a, 0, 0, 0, 2, 0, 0, 0, 0x14, 0, 0, 0],
                     ZD_LITTLE, 4, SPECIFICATION)
        extent = Int64(read_dheader(r))
        end_at = reader_position(r) + extent
        mark = reader_save(r)
        pairs = 0
        while reader_position(r) < end_at && reader_status(r) === ZD_STATUS_OK
            read_i32(r)
            read_i32(r)
            pairs += 1
        end
        reader_refuse(r, ZD_STATUS_OUT_OF_RANGE)
        reader_restore(r, mark)
        @test pairs == 2
        @test [read_i32(r), read_i32(r), read_i32(r), read_i32(r)] == Int32[1, 10, 2, 20]
        @test reader_position(r) == 20
        @test reader_status(r) === ZD_STATUS_OK
    end
end
