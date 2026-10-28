/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * D6 and G16 through Fast DDS's own serialiser.
 *
 * O1 already records that Fast DDS truncates a bounded string in silence
 * and enforces no sequence bound at all. This probe is the same
 * measurement taken at the two rows that ask for it rather than at the
 * one that mentions it, with the octets beside it: what a truncation and
 * an unenforced bound do to the members *behind* them is the half a
 * `try_construct` row does not show.
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

static void write_sample(const char *label, const Sample &sample) {
    SamplePubSubType support;
    SerializedPayload_t payload(4096);
    Sample copy = sample;
    if (!support.serialize(&copy, payload,
                           DataRepresentationId_t::XCDR2_DATA_REPRESENTATION)) {
        std::printf("fastdds\t%s\tREFUSED\t\n", label);
        return;
    }
    std::printf("fastdds\t%s\twrote\t", label);
    for (std::uint32_t i = 4; i < payload.length; i++)
        std::printf("%02x", payload.data[i]);
    std::printf("\n");
}

int main() {
    Sample sample;
    sample.s("abcd");
    sample.v({7, 9});
    sample.tail(42);
    write_sample("in-bound", sample);

    /* D6. `fixed_string<4>` is the holder, so the assignment decides. */
    sample.s("abcdefgh");
    std::printf("fastdds\tstring-holder\tassigned 8 characters, holder now \"%s\" (%zu)\t\n",
                sample.s().c_str(), sample.s().size());
    write_sample("string-8-of-4", sample);

    /* G16. The bound is not in the holder: `sequence<long,2>` is a
       `std::vector<int32_t>` like any unbounded one. */
    sample.s("abcd");
    sample.v({7, 9, 11});
    std::printf("fastdds\tvector-size\t%zu elements in a sequence<long,2>\t\n",
                sample.v().size());
    write_sample("sequence-3-of-2", sample);
    return 0;
}
