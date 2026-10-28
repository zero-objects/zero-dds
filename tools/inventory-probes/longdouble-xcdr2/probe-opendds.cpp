/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * C7 — the same question through OpenDDS, and B1 beside it.
 *
 * OpenDDS is the one implementation here whose serialiser takes its byte
 * order as a run-time argument rather than from the host, so the same
 * binary on this x86-64 machine writes the big-endian form. That is what
 * makes `D_CDR2_BE` reachable: `EncapsulationHeader(Encoding(KIND_XCDR2,
 * ENDIAN_BIG), extensibility)` is OpenDDS's own derivation of the
 * identifier from the type's extensibility, and it is the tenth of
 * Table 60's ten that no run in this tree had ever seen.
 */
#include <cstdio>
#include <cstring>

#include <dds/DCPS/Serializer.h>
#include <dds/DCPS/EncapsulationHeader.h>
#include <ace/Message_Block.h>

#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::EncapsulationHeader;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_BIG;
using OpenDDS::DCPS::ENDIAN_LITTLE;

static void dump(const char *version, const char *endian, Encoding::Kind kind,
                 OpenDDS::DCPS::Endianness endianness) {
    Sample sample;
    sample.head = 1;
    sample.d = 1.5L;
    sample.tail = 2;
    Encoding enc(kind, endianness);
    size_t room = OpenDDS::DCPS::serialized_size(enc, sample) + 64;
    ACE_Message_Block mb(room);
    Serializer ser(&mb, enc);
    if (!(ser << sample)) {
        std::printf("opendds\t%s\t%s\t!OpenDDS refused to serialise the sample\n",
                    version, endian);
        return;
    }
    const size_t length = mb.length();
    std::printf("opendds\t%s\t%s\tlength=%zu\tpayload=", version, endian, length);
    for (size_t i = 0; i < length; i++)
        std::printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    /* As in the Fast DDS probe: the member's own octets, so the offset
       is read off the payload rather than counted from an assumption
       about where the padding went. */
    unsigned char raw[sizeof(long double)];
    long double value = 1.5L;
    std::memcpy(raw, &value, sizeof raw);
    std::printf("\tmember=");
    for (size_t i = 0; i < sizeof raw; i++) std::printf("%02x", raw[i]);
    std::printf("\tsizeof(long double)=%zu\n", sizeof(long double));
}

int main() {
    dump("xcdr1", "le", Encoding::KIND_XCDR1, ENDIAN_LITTLE);
    dump("xcdr1", "be", Encoding::KIND_XCDR1, ENDIAN_BIG);
    dump("xcdr2", "le", Encoding::KIND_XCDR2, ENDIAN_LITTLE);
    dump("xcdr2", "be", Encoding::KIND_XCDR2, ENDIAN_BIG);
    return 0;
}
