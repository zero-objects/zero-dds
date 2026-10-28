/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * One sample through OpenDDS's own Serializer.
 *
 * OpenDDS is a fourth DDS implementation and the only one of the five
 * with CORBA/TAO lineage, so it answers questions the other three
 * cannot parse — `map` above all. Nothing here decides a wire question.
 *
 * Unlike the other three this template needs no byte-order substitution:
 * `OpenDDS::DCPS::Encoding` takes the endianness as a runtime argument,
 * so one binary produces all four rows and `gate.sh` selects the ones
 * the run asked for.
 *
 * `Serializer` writes no encapsulation header, so every byte printed
 * here is body.
 */
#include <cstdio>
#include <dds/DCPS/Serializer.h>
#include <ace/Message_Block.h>
#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_LITTLE;
using OpenDDS::DCPS::ENDIAN_BIG;

static void dump(const char *encoding, const char *endian, Encoding::Kind kind,
                 OpenDDS::DCPS::Endianness endianness, const @TYPE@ &sample) {
    Encoding enc(kind, endianness);
    ACE_Message_Block mb(65536);
    Serializer ser(&mb, enc);
    if (!(ser << sample)) {
        printf("%s\t%s\trefused\t\t\tOpenDDS refused to serialise the sample\n",
               encoding, endian);
        return;
    }
    const size_t length = mb.length();
    printf("%s\t%s\twrote\t%zu\t\t", encoding, endian, length);
    for (size_t i = 0; i < length; i++)
        printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    printf("\n");
}

int main() {
    @TYPE@ sample;
#include "fill.inc"
    dump("xcdr1", "le", Encoding::KIND_XCDR1, ENDIAN_LITTLE, sample);
    dump("xcdr2", "le", Encoding::KIND_XCDR2, ENDIAN_LITTLE, sample);
    dump("xcdr1", "be", Encoding::KIND_XCDR1, ENDIAN_BIG, sample);
    dump("xcdr2", "be", Encoding::KIND_XCDR2, ENDIAN_BIG, sample);
    return 0;
}
