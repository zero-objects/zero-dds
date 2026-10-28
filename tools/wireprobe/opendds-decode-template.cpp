/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The OpenDDS probe's read direction. The payload comes in on stdin with
 * no encapsulation header and the values go to stdout as canonical JSON.
 *
 * The same `Serializer` the write direction uses, in the other
 * direction, so a disagreement between the two is OpenDDS's and not a
 * disagreement between two different readers of it. Its generated
 * `operator>>` is what decides where a delimiter ends and how a
 * parameter list is walked; nothing here decides any of it.
 *
 * The dump statements are the CORBA C++ ones, as TAO's are, because
 * `opendds_idl --idl-version 4` plus `tao_idl -Sa -St` produces the
 * classic C++ mapping: a member is read out of the struct rather than
 * through an accessor.
 *
 * The encoding version and the byte order come in on the command line,
 * so one build per case answers every combination: `Encoding` takes both
 * as constructor arguments and neither reaches the generated
 * `operator>>` as a template parameter.
 *
 * A payload OpenDDS refuses is exit 2 and not exit 1. The stream is
 * wrong, which is a different finding from the probe being unable to
 * run, and a run that recorded the two as one would report a vendor's
 * rejection as a harness failure or the reverse.
 */
#include <cstdio>
#include <cstring>
#include <vector>

#include <ace/Message_Block.h>
#include <dds/DCPS/Serializer.h>

#include "genTypeSupportImpl.h"
#include "corbajson.hpp"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_BIG;
using OpenDDS::DCPS::ENDIAN_LITTLE;

int main(int argc, char **argv) {
    if (argc != 3) {
        std::fprintf(stderr, "usage: probe <le|be> <1|2>\n");
        return 2;
    }
    const bool big = std::strcmp(argv[1], "be") == 0;
    const bool second = std::strcmp(argv[2], "2") == 0;

    std::vector<char> raw;
    char chunk[4096];
    std::size_t got;
    while ((got = std::fread(chunk, 1, sizeof chunk, stdin)) > 0) {
        raw.insert(raw.end(), chunk, chunk + got);
    }

    Encoding encoding(second ? Encoding::KIND_XCDR2 : Encoding::KIND_XCDR1,
                      big ? ENDIAN_BIG : ENDIAN_LITTLE);
    /* `ACE_Message_Block` over the bytes without copying them: the block
     * does not own this memory and must not free it, which is what the
     * `DONT_DELETE` flag says. */
    ACE_Message_Block mb(raw.empty() ? chunk : &raw[0], raw.size());
    mb.wr_ptr(raw.size());
    Serializer serializer(&mb, encoding);

    @TYPE@ sample;
    if (!(serializer >> sample)) {
        std::fprintf(stderr, "opendds refused the payload\n");
        return 1;
    }

#include "dump.inc"
    return 0;
}
