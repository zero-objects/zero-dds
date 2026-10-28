/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The Fast DDS probe's read direction. `wireprobe-fastdds` substitutes
 * @TYPE@ with the C++ name, then compiles this
 * against `fastddsgen`'s output.
 *
 * The payload comes in on stdin with no encapsulation header and the
 * values go to stdout as canonical JSON.
 *
 * ## Where the encapsulation header comes from
 *
 * The generated `<T>PubSubType::deserialize` reads one and takes both
 * the byte order and the encoding flag from it, so the probe has to put
 * one back. The flag is not written from a table: a default-constructed
 * sample is serialised through the same generated type support first,
 * and the flag is read back out of the header that writes — PLAIN_CDR /
 * PLAIN_CDR2 for a final or appendable type, PL_CDR / PL_CDR2 for a
 * mutable one, which is a per-type decision `fastddsgen` made and not
 * one this probe is entitled to make. It is the same round the write
 * direction does, for the same reason.
 */
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <fastcdr/Cdr.h>
#include <fastcdr/CdrEncoding.hpp>

#include "gen.hpp"
#include "genCdrAux.hpp"
#include "genPubSubTypes.hpp"
#include "dumpjson.hpp"
#include "vendor_crash.h"

using eprosima::fastdds::dds::DataRepresentationId_t;
using eprosima::fastdds::rtps::SerializedPayload_t;

namespace {

constexpr std::uint32_t CAPACITY = 262144;

} // namespace

/* The representation and the byte order come in on the command line and
 * not from `sed`: they used to be part of the binary's identity, so the
 * four combinations of one case ran `fastddsgen` four times over
 * identical generated code. Both are run-time values in Fast CDR. */
int main(int argc, char **argv) {
    if (argc != 3) {
        std::fprintf(stderr, "usage: probe <le|be> <1|2>\n");
        return 2;
    }
    const bool big = std::strcmp(argv[1], "be") == 0;
    const bool second = std::strcmp(argv[2], "2") == 0;
    const DataRepresentationId_t representation =
        second ? DataRepresentationId_t::XCDR2_DATA_REPRESENTATION
               : DataRepresentationId_t::XCDR_DATA_REPRESENTATION;

    @TYPE@ sample;
    @TYPE@PubSubType support;

    /* The flag `fastddsgen` chose for this type, read out of a header it
       wrote itself. The sample is filled first, not default-constructed:
       an `@external` member of a default sample is a null
       `eprosima::fastcdr::external`, which Fast DDS refuses to
       serialise, and the refusal used to take the whole read direction
       with it. The filled values themselves do not matter to the flag
       and do not survive — the sample is reset to a fresh one before it
       becomes the deserialisation target.

       Everything from here to the JSON dump runs under one catch: a
       fastcdr exception out of filling the sample, out of the
       flag-probe serialise, or out of the real deserialise is Fast
       DDS's own refusal of this case, not a crash. Left uncaught, any
       of the three takes the process down with SIGABRT, which the
       pairwise runner reads as an infrastructure failure rather than a
       vendor refusal — 28 such aborts in the 2026-09-21 nightly, all
       from the unguarded flag-probe serialise on this path. */
    try {
#include "fill.inc"
        SerializedPayload_t probe(CAPACITY);
        if (!support.serialize(&sample, probe, representation) || probe.length < 4) {
            std::fprintf(stderr, "fast dds wrote no encapsulation header to read the flag from\n");
            return 1;
        }
        const unsigned flag = static_cast<unsigned>(probe.data[1]) & 0xFEu;
        sample = @TYPE@();

        std::vector<unsigned char> raw(CAPACITY);
        const std::size_t length = std::fread(raw.data() + 4, 1, raw.size() - 4, stdin);
        if (std::feof(stdin) == 0) {
            std::fprintf(stderr, "the payload is larger than the probe's buffer\n");
            return 1;
        }
        raw[0] = 0;
        raw[1] = static_cast<unsigned char>(flag | (big ? 0u : 1u));
        raw[2] = 0;
        raw[3] = 0;

        SerializedPayload_t payload(static_cast<std::uint32_t>(length + 4));
        std::memcpy(payload.data, raw.data(), length + 4);
        payload.length = static_cast<std::uint32_t>(length + 4);
        payload.pos = 0;

        zd_vendor_call("fast dds", "reading the payload");
        const bool read = support.deserialize(payload, &sample);
        zd_vendor_return();
        if (!read) {
            std::fprintf(stderr, "fast dds refused to deserialise the payload\n");
            return 1;
        }
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        zd_vendor_return();
        std::fprintf(stderr, "fast cdr: %s\n", error.what());
        return 1;
    }

    /* A union Fast DDS read can hold a discriminator that selects no
       member it holds: read from bytes it took for another layout, `_d()`
       names a branch whose getter then throws BadParamException. The
       value is not there to be shown, which is a refusal and not a crash
       of this probe. */
    try {
#include "dump.inc"
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        std::fprintf(stderr, "fast dds holds no value to show: %s\n", error.what());
        return 1;
    }
    return 0;
}
