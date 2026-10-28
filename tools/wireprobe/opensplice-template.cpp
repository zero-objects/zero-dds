/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * One IDL type and one JSON sample through OpenSplice's own serializer:
 * `DDS::OpenSplice::CdrTypeSupport::serialize`, the entry point the
 * product offers for exactly this (Community Edition 6.9, SACPP).
 *
 * OpenSplice 6.9 has one representation, XCDR1, and writes it in the
 * host's byte order; `CdrTypeSupport` takes neither as an argument. So
 * this binary prints the one row OpenSplice answers — `xcdr1` in the
 * host's order — and a refusal for the other three, each a sentence and
 * not an absent line a reader would have to interpret.
 *
 * `serialize` needs the type registered with a participant. The
 * participant comes from the single-process configuration without a
 * network (`ospl_sp_no_network.xml`, which `wireprobe-opensplice` names
 * in OSPL_URI): nothing leaves the process.
 *
 * The serializer writes the four-byte encapsulation header in front of
 * the body. The header is checked — it has to say CDR in the host's
 * order, or the bytes are not what the row claims — and not printed:
 * every byte printed here is body, which is what `--emit payload` means
 * everywhere else in this suite.
 *
 * The fill statements are the CORBA C++ mapping's (`wireprobe-fill
 * --vendor omniorb`). SACPP is that mapping, and its headers declare the
 * CORBA namespace the statements name (CORBA::Octet, CORBA::string_dup).
 *
 * Nothing here decides a wire question.
 */
#include <cstdio>
#include <cstring>
#include <vector>

#include "ccpp_dds_dcps.h"
#include "ccpp_gen.h"
#include "CdrTypeSupport.h"

static const bool host_is_little = [] {
    const unsigned short one = 1;
    unsigned char first;
    std::memcpy(&first, &one, 1);
    return first == 1;
}();

int main() {
    DDS::DomainParticipantFactory_var factory = DDS::DomainParticipantFactory::get_instance();
    DDS::DomainParticipant_var participant = factory->create_participant(
        DDS::DOMAIN_ID_DEFAULT, PARTICIPANT_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
    if (!participant.in()) {
        fprintf(stderr, "no OpenSplice participant (OSPL_URI?)\n");
        return 4;
    }
    @TYPE@TypeSupport_var type_support = new @TYPE@TypeSupport;
    if (type_support->register_type(participant.in(), "ProbeType") != DDS::RETCODE_OK) {
        fprintf(stderr, "OpenSplice did not register the type\n");
        return 4;
    }

    @TYPE@ sample;
#include "fill.inc"

    const char *host = host_is_little ? "le" : "be";
    const char *other = host_is_little ? "be" : "le";
    DDS::OpenSplice::CdrTypeSupport cdr(*type_support.in());
    DDS::OpenSplice::CdrSerializedData *serialized = NULL;
    if (cdr.serialize(&sample, &serialized) != DDS::RETCODE_OK || serialized == NULL) {
        printf("xcdr1\t%s\t!OpenSplice refused to serialise the sample\n", host);
    } else {
        std::vector<unsigned char> bytes(serialized->get_size());
        serialized->get_data(bytes.data());
        delete serialized;
        const unsigned char want = host_is_little ? 0x01 : 0x00;
        if (bytes.size() < 4 || bytes[0] != 0x00 || bytes[1] != want) {
            printf("xcdr1\t%s\t!the encapsulation header is not CDR in the host's order\n", host);
        } else {
            printf("xcdr1\t%s\t", host);
            for (size_t i = 4; i < bytes.size(); i++) printf("%02x", bytes[i]);
            printf("\n");
        }
    }
    printf("xcdr1\t%s\t!OpenSplice 6.9 serialises in the host's byte order only\n", other);
    printf("xcdr2\tle\t!OpenSplice 6.9 writes XCDR1 only\n");
    printf("xcdr2\tbe\t!OpenSplice 6.9 writes XCDR1 only\n");
    factory->delete_participant(participant.in());
    return 0;
}
