/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The whole corpus through OpenDDS's `Serializer`, in one program.
 *
 * `wireprobe-batch opendds` writes every case's IDL into one `gen.idl`,
 * each in a module of its own, runs `tao_idl -Sa -St` and `opendds_idl`
 * once over it, and generates one `zd_case_<n>` function per case from
 * this template's `@CASES@` marker. One compile, one link, one process,
 * every row.
 *
 * `Encoding` carries the version and the byte order, so a template
 * function serves every case and every combination: nothing about the
 * four is a compile-time decision, and nothing about the case is either
 * beyond the type it is instantiated for.
 *
 * `Serializer` writes no encapsulation header, so every octet printed
 * is body — which is what `--emit payload` means everywhere else here.
 *
 * Nothing here decides a wire question.
 */
#include <cstdio>
#include <cstring>

#include <ace/Message_Block.h>
#include <dds/DCPS/Serializer.h>

#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_BIG;
using OpenDDS::DCPS::ENDIAN_LITTLE;

/* One case, one combination. A refusal is a row and not a silence. */
template <typename T>
void zd_row(const char *name, const char *version, const char *order,
            Encoding::Kind kind, OpenDDS::DCPS::Endianness endianness,
            const T &sample) {
    Encoding encoding(kind, endianness);
    /* Sized from the sample rather than fixed: a value larger than one
       block would be serialised into a chain and `rd_ptr()` would print
       the first link only, which is a truncated payload wearing a
       vendor's name. */
    const size_t room = OpenDDS::DCPS::serialized_size(encoding, sample) + 64;
    ACE_Message_Block mb(room);
    Serializer serializer(&mb, encoding);
    if (!(serializer << sample)) {
        std::printf("%s\t%s\t%s\t!OpenDDS refused to serialise the sample\n",
                    name, version, order);
        return;
    }
    if (mb.cont() != 0) {
        std::printf("%s\t%s\t%s\t!the sample did not fit one block\n", name, version, order);
        return;
    }
    const size_t length = mb.length();
    std::printf("%s\t%s\t%s\t", name, version, order);
    for (size_t i = 0; i < length; i++)
        std::printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    std::printf("\n");
}

/* All four combinations of one case. */
template <typename T>
void zd_emit(const char *name, const T &sample) {
    zd_row(name, "xcdr1", "le", Encoding::KIND_XCDR1, ENDIAN_LITTLE, sample);
    zd_row(name, "xcdr1", "be", Encoding::KIND_XCDR1, ENDIAN_BIG, sample);
    zd_row(name, "xcdr2", "le", Encoding::KIND_XCDR2, ENDIAN_LITTLE, sample);
    zd_row(name, "xcdr2", "be", Encoding::KIND_XCDR2, ENDIAN_BIG, sample);
}

@CASES@

int main() {
    /* Line buffered, so a case that kills the process cannot take the
     * rows of the cases before it down with it. */
    std::setvbuf(stdout, nullptr, _IOLBF, 0);
@CALLS@
    return 0;
}
