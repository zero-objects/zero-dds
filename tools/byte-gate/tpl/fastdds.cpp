/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The same sample through eProsima Fast DDS's own serialiser (Fast CDR).
 *
 * `gate.sh` substitutes @TYPE@ with the generated C++ name and
 * @ENDIANNESS@ with `LITTLE_ENDIANNESS` or `BIG_ENDIANNESS`.
 *
 * Fast CDR writes a four-byte encapsulation header and this harness is
 * the thing that asks it to — `serialize_encapsulation()` is called
 * here, not by the generated code. The header is therefore reported in
 * its own column rather than in the body, because the body is the
 * vendor's answer and the header is partly this file's.
 */
#include <cstdio>
#include <fastcdr/Cdr.h>
#include <fastcdr/FastBuffer.h>
#include "gen.hpp"
#include "genCdrAux.hpp"

using eprosima::fastcdr::Cdr;
using eprosima::fastcdr::CdrVersion;
using eprosima::fastcdr::FastBuffer;

static void dump(const char *encoding, CdrVersion version, const @TYPE@ &sample) {
    char raw[65536];
    FastBuffer buffer(raw, sizeof(raw));
    Cdr ser(buffer, Cdr::@ENDIANNESS@, version);
    ser.serialize_encapsulation();
    ser << sample;
    const size_t length = ser.get_serialized_data_length();
    if (length < 4) {
        printf("%s\t@ENDIAN@\trefused\t\t\tFast CDR wrote no encapsulation header\n",
               encoding);
        return;
    }
    printf("%s\t@ENDIAN@\twrote\t%zu\t%02x%02x\t", encoding, length - 4,
           static_cast<unsigned char>(raw[0]), static_cast<unsigned char>(raw[1]));
    for (size_t i = 4; i < length; i++)
        printf("%02x", static_cast<unsigned char>(raw[i]));
    printf("\n");
}

int main() {
    @TYPE@ sample;
#include "fill.inc"
    dump("xcdr1", CdrVersion::XCDRv1, sample);
    dump("xcdr2", CdrVersion::XCDRv2, sample);
    return 0;
}
