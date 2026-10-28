/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The OpenSplice probe's read direction. The payload comes in on stdin
 * with no encapsulation header and the values go to stdout as canonical
 * JSON.
 *
 * The same `CdrTypeSupport` the write direction uses, in the other
 * direction: `deserialize` takes the serialized message as `serialize`
 * produced it, header included, so this puts the header of the one
 * representation OpenSplice 6.9 has — CDR in the host's byte order — in
 * front of the bytes it reads. A request for XCDR2 or for the other byte
 * order is refused before anything is read: there is no such header
 * OpenSplice would accept, and translating the stream first would be
 * this probe answering instead of OpenSplice.
 *
 * The dump statements are the CORBA C++ ones (`wireprobe-dump --vendor
 * omniorb`); SACPP is that mapping.
 *
 * A payload OpenSplice refuses is exit 2 and not exit 1: the stream is
 * wrong, which is a different finding from the probe being unable to
 * run.
 */
#include <cstdio>
#include <cstring>
#include <vector>

#include "ccpp_dds_dcps.h"
#include "ccpp_gen.h"
#include "CdrTypeSupport.h"

/* SACPP's CORBA namespace has no WChar: OpenSplice 6.9 has no wchar at
 * all (`idlpp` refuses the type), so no type this probe can build has a
 * wchar member and corbajson.hpp's wchar printers are never called. The
 * name is given here only so that header compiles; it is the width
 * OpenSplice's own kernel uses for c_wchar (os_short). */
namespace CORBA { typedef DDS::UShort WChar; }

#include "corbajson.hpp"

static const bool host_is_little = [] {
    const unsigned short one = 1;
    unsigned char first;
    std::memcpy(&first, &one, 1);
    return first == 1;
}();

int main(int argc, char **argv) {
    if (argc != 3) {
        std::fprintf(stderr, "usage: probe <le|be> <1|2>\n");
        return 2;
    }
    const bool big = std::strcmp(argv[1], "be") == 0;
    if (std::strcmp(argv[2], "1") != 0) {
        std::fprintf(stderr, "OpenSplice 6.9 reads XCDR1 only\n");
        return 3;
    }
    if (big == host_is_little) {
        std::fprintf(stderr, "OpenSplice 6.9 reads the host's byte order only\n");
        return 3;
    }

    std::vector<unsigned char> raw;
    raw.push_back(0x00);
    raw.push_back(host_is_little ? 0x01 : 0x00);
    raw.push_back(0x00);
    raw.push_back(0x00);
    unsigned char chunk[4096];
    std::size_t got;
    while ((got = std::fread(chunk, 1, sizeof chunk, stdin)) > 0) {
        raw.insert(raw.end(), chunk, chunk + got);
    }

    DDS::DomainParticipantFactory_var factory = DDS::DomainParticipantFactory::get_instance();
    DDS::DomainParticipant_var participant = factory->create_participant(
        DDS::DOMAIN_ID_DEFAULT, PARTICIPANT_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
    if (!participant.in()) {
        std::fprintf(stderr, "no OpenSplice participant (OSPL_URI?)\n");
        return 4;
    }
    @TYPE@TypeSupport_var type_support = new @TYPE@TypeSupport;
    if (type_support->register_type(participant.in(), "ProbeType") != DDS::RETCODE_OK) {
        std::fprintf(stderr, "OpenSplice did not register the type\n");
        return 4;
    }
    DDS::OpenSplice::CdrTypeSupport cdr(*type_support.in());

    @TYPE@ sample;
    if (cdr.deserialize(raw.data(), static_cast<unsigned int>(raw.size()), &sample) != DDS::RETCODE_OK) {
        std::fprintf(stderr, "opensplice refused the payload\n");
        factory->delete_participant(participant.in());
        return 1;
    }

#include "dump.inc"
    factory->delete_participant(participant.in());
    return 0;
}
