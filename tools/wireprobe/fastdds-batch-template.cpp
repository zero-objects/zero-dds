/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The whole corpus through Fast DDS's serialiser, in one program.
 *
 * `wireprobe-batch fastdds` writes every case's IDL into one `gen.idl`,
 * each in a module of its own, runs `fastddsgen` once over it, and
 * generates one `zd_case_<n>` function per case from this template's
 * `@CASES@` marker. One compile, one link, one process, every row.
 *
 * `fastddsgen` is the slowest generator of the five — a JVM start per
 * run — and the per-case arrangement paid for it 335 times to answer
 * questions that differ only in which sample is filled in.
 *
 * ## Who owns the encapsulation header
 *
 * The same two passes the per-case template makes, and for the same
 * reason. The flag lives in the generated `<T>PubSubType::serialize`,
 * which is the code Fast DDS itself runs, and it is a per-type
 * decision — PLAIN_CDR / PLAIN_CDR2 for a final or appendable type,
 * PL_CDR / PL_CDR2 for a mutable one. So the first pass is that
 * generated function, and the encapsulation identifier it writes is
 * where the flag comes back out: the identifiers are pairs, big endian
 * even and little endian odd, and the even member of each pair *is* the
 * `EncodingAlgorithmFlag` value.
 *
 * The generated function serialises in the host's byte order and takes
 * no say in it, which is the one thing the probe has to choose, so the
 * second pass does the emitting with the flag the first pass reported.
 * In little endian the two passes must come out byte for byte
 * identical; they are the same vendor code over the same sample, and a
 * disagreement is reported as a refusal rather than emitted.
 *
 * Nothing here decides a wire question.
 */
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include <fastcdr/Cdr.h>
#include <fastcdr/CdrEncoding.hpp>
#include <fastcdr/FastBuffer.h>

#include "gen.hpp"
#include "genCdrAux.hpp"
#include "genPubSubTypes.hpp"

using eprosima::fastcdr::Cdr;
using eprosima::fastcdr::CdrVersion;
using eprosima::fastcdr::EncodingAlgorithmFlag;
using eprosima::fastcdr::FastBuffer;
using eprosima::fastdds::dds::DataRepresentationId_t;
using eprosima::fastdds::rtps::SerializedPayload_t;

namespace {

/* Only has to be larger than the corpus; both passes are bounded by it
   and Fast CDR throws rather than overruns. */
constexpr std::uint32_t CAPACITY = 262144;

/* One case, one combination. A refusal is a row and not a silence. */
template <typename T, typename Support>
void zd_row(const char *name, const char *version, const char *order,
            DataRepresentationId_t representation, CdrVersion cdr_version,
            Cdr::Endianness endianness, T &sample) {
    Support support;
    SerializedPayload_t vendor(CAPACITY);
    /* A case Fast DDS refuses can throw out of this call rather than
       return false — an out-of-range union discriminator does — and
       an uncaught throw here would take the whole batch process down
       with it, losing every row the cases before this one already
       printed along with it. */
    try {
        if (!support.serialize(&sample, vendor, representation)) {
            std::printf("%s\t%s\t%s\t!fast dds refused to serialise the sample\n",
                        name, version, order);
            return;
        }
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        std::printf("%s\t%s\t%s\t!fast cdr: %s\n", name, version, order, error.what());
        return;
    }
    if (vendor.length < 4) {
        std::printf("%s\t%s\t%s\t!fast dds wrote no encapsulation header\n",
                    name, version, order);
        return;
    }
    const EncodingAlgorithmFlag flag =
        static_cast<EncodingAlgorithmFlag>(vendor.data[1] & 0xFE);

    std::vector<char> raw(CAPACITY);
    FastBuffer buffer(raw.data(), raw.size());
    Cdr serialiser(buffer, endianness, cdr_version);
    if (!serialiser.set_encoding_flag(flag)) {
        std::printf("%s\t%s\t%s\t!fast cdr refused the encoding flag\n",
                    name, version, order);
        return;
    }
    std::size_t length = 0;
    try {
        serialiser.serialize_encapsulation();
        serialiser << sample;
        serialiser.set_dds_cdr_options({0, 0});
        length = serialiser.get_serialized_data_length();
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        std::printf("%s\t%s\t%s\t!fast cdr: %s\n", name, version, order, error.what());
        return;
    }
    if (length < 4) {
        std::printf("%s\t%s\t%s\t!fast cdr wrote no encapsulation header\n",
                    name, version, order);
        return;
    }
    if (Cdr::LITTLE_ENDIANNESS == serialiser.endianness() &&
        (vendor.length != length ||
         std::memcmp(vendor.data, raw.data(), length) != 0)) {
        std::printf("%s\t%s\t%s\t!the two passes disagree in little endian: the encoding "
                    "flag read back from the header is wrong\n", name, version, order);
        return;
    }

    /* `serialize_encapsulation` wrote the four-byte header. `--emit
       payload` is the payload, so strip exactly that and nothing else. */
    std::printf("%s\t%s\t%s\t", name, version, order);
    for (std::size_t i = 4; i < length; i++)
        std::printf("%02x", static_cast<unsigned char>(raw[i]));
    std::printf("\n");
}

} // namespace

/* All four combinations of one case. */
template <typename T, typename Support>
void zd_emit(const char *name, T &sample) {
    zd_row<T, Support>(name, "xcdr1", "le", DataRepresentationId_t::XCDR_DATA_REPRESENTATION,
                       CdrVersion::XCDRv1, Cdr::LITTLE_ENDIANNESS, sample);
    zd_row<T, Support>(name, "xcdr1", "be", DataRepresentationId_t::XCDR_DATA_REPRESENTATION,
                       CdrVersion::XCDRv1, Cdr::BIG_ENDIANNESS, sample);
    zd_row<T, Support>(name, "xcdr2", "le", DataRepresentationId_t::XCDR2_DATA_REPRESENTATION,
                       CdrVersion::XCDRv2, Cdr::LITTLE_ENDIANNESS, sample);
    zd_row<T, Support>(name, "xcdr2", "be", DataRepresentationId_t::XCDR2_DATA_REPRESENTATION,
                       CdrVersion::XCDRv2, Cdr::BIG_ENDIANNESS, sample);
}

@CASES@

int main() {
    /* Line buffered, so a case that kills the process cannot take the
     * rows of the cases before it down with it. */
    std::setvbuf(stdout, nullptr, _IOLBF, 0);
@CALLS@
    return 0;
}
