/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * C7 — where a `long double` sits and how wide it is, in Fast DDS's own
 * output, under both encoding versions.
 *
 * `wireprobe-fastdds` cannot answer this: `wireprobe-fill` refuses the
 * member by name, because the *value* a C++ `long double` holds is the
 * compiler's x87 extended and not §9.3.1.3's binary128, and a run that
 * compared values would be comparing the platform. The layout is a
 * different question from the value and this probe asks only the
 * layout: `head` is at 0, `tail` is the last octet, and everything
 * between them is the member's padding and its width.
 *
 * The value is set from a `double` so that the octets are reproducible
 * on this host; nothing here claims they are what the clause asks for.
 */
#include <cstdio>
#include <cstring>

#include <fastcdr/Cdr.h>
#include <fastcdr/CdrEncoding.hpp>

#include "gen.hpp"
#include "genCdrAux.hpp"
#include "genPubSubTypes.hpp"

using eprosima::fastdds::dds::DataRepresentationId_t;
using eprosima::fastdds::rtps::SerializedPayload_t;

static void dump(const char *version, DataRepresentationId_t representation) {
    Sample sample;
    sample.head(1);
    sample.d(1.5L);
    sample.tail(2);
    SamplePubSubType support;
    SerializedPayload_t payload(4096);
    if (!support.serialize(&sample, payload, representation)) {
        std::printf("fastdds\t%s\t!Fast DDS refused to serialise the sample\n", version);
        return;
    }
    /* `serialize` writes the four-octet encapsulation header first; the
       payload this probe reports is the body behind it, which is what
       `--emit payload` means everywhere else in the suite. */
    std::printf("fastdds\t%s\theader=%02x%02x\tlength=%u\tpayload=", version,
                payload.data[0], payload.data[1],
                static_cast<unsigned>(payload.length - 4));
    for (std::uint32_t i = 4; i < payload.length; i++)
        std::printf("%02x", payload.data[i]);
    /* The member's own octets, so its offset in the payload is read off
       and not inferred: a `long double` on x86-64 is ten significant
       octets in sixteen, and which six are padding is the compiler's. */
    unsigned char raw[sizeof(long double)];
    long double value = 1.5L;
    std::memcpy(raw, &value, sizeof raw);
    std::printf("\tmember=");
    for (std::size_t i = 0; i < sizeof raw; i++) std::printf("%02x", raw[i]);
    std::printf("\tsizeof(long double)=%zu\n", sizeof(long double));
}

int main() {
    dump("xcdr1", DataRepresentationId_t::XCDR_DATA_REPRESENTATION);
    dump("xcdr2", DataRepresentationId_t::XCDR2_DATA_REPRESENTATION);
    return 0;
}
