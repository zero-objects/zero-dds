/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * D6 and G16 through OpenDDS's own serialiser.
 *
 * OpenDDS is the fourth answer to both: its `string<4>` is a
 * `TAO::String_Manager`, which holds any length, and its
 * `sequence<long,2>` is a `TAO::bounded_value_sequence<Long,2>`, which
 * carries the bound in the type. So the two halves are answered in two
 * different places in the same implementation, and the probe asks both.
 */
#include <cstdio>

#include <dds/DCPS/Serializer.h>
#include <ace/Message_Block.h>

#include "genTypeSupportImpl.h"

using OpenDDS::DCPS::Encoding;
using OpenDDS::DCPS::Serializer;
using OpenDDS::DCPS::ENDIAN_LITTLE;

static void write_sample(const char *label, const Sample &sample) {
    Encoding enc(Encoding::KIND_XCDR2, ENDIAN_LITTLE);
    size_t room = OpenDDS::DCPS::serialized_size(enc, sample) + 256;
    ACE_Message_Block mb(room);
    Serializer ser(&mb, enc);
    if (!(ser << sample)) {
        std::printf("opendds\t%s\tREFUSED\t\n", label);
        return;
    }
    std::printf("opendds\t%s\twrote\t", label);
    const size_t length = mb.length();
    for (size_t i = 0; i < length; i++)
        std::printf("%02x", static_cast<unsigned char>(mb.rd_ptr()[i]));
    std::printf("\n");
}

/* Every call below can raise a CORBA system exception, and which one it
   raises is the measurement — an unhandled `BAD_PARAM` would abort the
   process and lose the answer it was raised to give. */
template <typename F>
static void say(const char *label, F body) {
    try {
        body();
    } catch (const CORBA::SystemException &e) {
        std::printf("opendds\t%s\tREFUSED\t%s\n", label, e._name());
    } catch (const std::exception &e) {
        std::printf("opendds\t%s\tREFUSED\t%s\n", label, e.what());
    }
}

int main() {
    Sample sample;
    sample.s = "abcd";
    sample.v.length(2);
    sample.v[0] = 7;
    sample.v[1] = 9;
    sample.tail = 42;
    write_sample("in-bound", sample);

    /* D6. `String_Manager` takes any length — the bound survives only in
       the IDL — so the serialiser is what decides. */
    say("string-holder", [&] {
        sample.s = "abcdefgh";
        std::printf("opendds\tstring-holder\tassigned 8 characters, holder now %u\t\n",
                    static_cast<unsigned>(std::char_traits<char>::length(sample.s.in())));
    });
    say("string-8-of-4", [&] { write_sample("string-8-of-4", sample); });

    /* G16. The bound is in the *type* here — `bounded_value_sequence<
       Long, 2>` — so `length(3)` is the container's own answer and it
       comes before any serialiser. */
    sample.s = "abcd";
    say("length(3)", [&] {
        sample.v.length(3);
        std::printf("opendds\tlength(3)\taccepted\tmaximum=%u length=%u\n",
                    sample.v.maximum(), sample.v.length());
        sample.v[2] = 11;
    });
    say("sequence-3-of-2", [&] { write_sample("sequence-3-of-2", sample); });
    return 0;
}
