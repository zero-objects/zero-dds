/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * B1 — the tenth encapsulation identifier, `D_CDR2_BE = 0x0008`.
 *
 * The row said it needed a big-endian machine. That was true of the
 * entry points the run had: RTI's `serialize_to_cdr_buffer_ex` and Fast
 * DDS's `PubSubType::serialize` both take the byte order from the host,
 * so on x86-64 neither can be asked for the big-endian half of a pair
 * whose two members differ only in that bit.
 *
 * OpenDDS's does not. `OpenDDS::DCPS::Encoding` carries the endianness
 * as a member and `Serializer` reads it from there, so `Encoding(kind,
 * ENDIAN_BIG)` on this host produces the big-endian stream and
 * `EncapsulationHeader(encoding, extensibility)` — OpenDDS's own
 * derivation, `EncapsulationHeader.cpp::from_encoding` — produces the
 * identifier that goes in front of it. Nothing here writes an
 * identifier from a table: every one printed is what OpenDDS derived.
 *
 * The sample is serialised behind each header so that the identifier is
 * not the only thing measured — an identifier that named a byte order
 * the body did not have would be a header this probe wrote rather than
 * one OpenDDS did.
 */
#include <cstdio>

#include <dds/DCPS/Serializer.h>
#include <dds/DCPS/EncapsulationHeader.h>
#include <ace/Message_Block.h>

#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::EncapsulationHeader;
using OpenDDS::DCPS::Extensibility;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_BIG;
using OpenDDS::DCPS::ENDIAN_LITTLE;

template <typename T>
static void dump(const char *type_name, const char *version, const char *endian,
                 Encoding::Kind kind, OpenDDS::DCPS::Endianness endianness,
                 Extensibility extensibility, const T &sample) {
    Encoding enc(kind, endianness);
    EncapsulationHeader header(enc, extensibility);
    if (!header.is_good()) {
        std::printf("%s\t%s\t%s\t!OpenDDS derived no identifier for this encoding\n",
                    type_name, version, endian);
        return;
    }
    size_t room = OpenDDS::DCPS::serialized_size(enc, sample) + 64;
    ACE_Message_Block mb(room);
    Serializer ser(&mb, enc);
    if (!(ser << sample)) {
        std::printf("%s\t%s\t%s\t!OpenDDS refused to serialise the sample\n",
                    type_name, version, endian);
        return;
    }
    /* The identifier is big-endian on the wire whatever the body is,
       DDSI-RTPS 2.5 §10.5, and `EncapsulationHeader::kind()` is already
       that integer. */
    std::printf("%s\t%s\t%s\t%04x\t%s\tpayload=", type_name, version, endian,
                static_cast<unsigned>(header.kind()), header.to_string().c_str());
    const size_t length = mb.length();
    for (size_t i = 0; i < length; i++)
        std::printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    std::printf("\n");
}

template <typename T>
static void all_four(const char *type_name, Extensibility extensibility, const T &sample) {
    dump(type_name, "xcdr1", "le", Encoding::KIND_XCDR1, ENDIAN_LITTLE, extensibility, sample);
    dump(type_name, "xcdr1", "be", Encoding::KIND_XCDR1, ENDIAN_BIG, extensibility, sample);
    dump(type_name, "xcdr2", "le", Encoding::KIND_XCDR2, ENDIAN_LITTLE, extensibility, sample);
    dump(type_name, "xcdr2", "be", Encoding::KIND_XCDR2, ENDIAN_BIG, extensibility, sample);
}

int main() {
    Fin fin; fin.a = 7; fin.b = 1.5;
    App app; app.a = 7; app.b = 1.5;
    Mut mut; mut.a = 7; mut.b = 1.5;
    all_four("Fin", OpenDDS::DCPS::FINAL, fin);
    all_four("App", OpenDDS::DCPS::APPENDABLE, app);
    all_four("Mut", OpenDDS::DCPS::MUTABLE, mut);
    return 0;
}
