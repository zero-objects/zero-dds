/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The Fast DDS probe's body. `wireprobe-fastdds` substitutes @TYPE@
 * with the C++ name and compiles this against `fastddsgen`'s output.
 *
 * The bytes go to stdout with nothing around them.
 *
 * ## Why the representation, the version and the byte order are
 * ## arguments
 *
 * They used to be three `sed` substitutions, which put them into the
 * binary's identity: four combinations of one case meant four runs of
 * `fastddsgen` and four compiles of identical generated code, and
 * `fastddsgen` is the slowest generator of the five. Fast CDR decides
 * all three at run time — `serialize` takes the representation, and
 * `Cdr`'s constructor takes the endianness and the version — so one
 * build now answers every combination and `--matrix` prints all four
 * rows from one process.
 *
 * ## Who owns the encapsulation header
 *
 * `/root/byte-gate/gate/tpl/fastdds.cpp` builds a `Cdr` and calls
 * `serialize_encapsulation()` on it directly. That header is the
 * harness's: the encoding flag it carries is whatever the `Cdr` was
 * constructed with, not the flag `fastddsgen` chose for the type. It is
 * why `vendor_bytes.md` does not report a Fast DDS header byte.
 *
 * The flag lives in the generated `<T>PubSubType::serialize`, which is
 * the code Fast DDS itself runs, and it is a per-type decision —
 * PLAIN_CDR / PLAIN_CDR2 for a final or appendable type, PL_CDR /
 * PL_CDR2 for a mutable one. So the first pass here is that generated
 * function. It writes the vendor's header, and the encapsulation
 * identifier in it is where the flag comes back out: the identifiers
 * are pairs, big endian even and little endian odd, and the even member
 * of each pair *is* the `EncodingAlgorithmFlag` value — PLAIN_CDR 0x0,
 * PL_CDR 0x2, PLAIN_CDR2 0x6, DELIMIT_CDR2 0x8, PL_CDR2 0xa
 * (`fastcdr/CdrEncoding.hpp`).
 *
 * The generated function serialises in the host's byte order and takes
 * no say in it, which is the one thing the probe has to choose, so the
 * second pass does the emitting with the flag the first pass reported.
 * In little endian the two passes must come out byte for byte
 * identical; they are the same vendor code over the same sample, and
 * the probe fails loudly if they are not.
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
#include "vendor_crash.h"

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

} // namespace

/* One combination, both passes. Returns the payload, or nothing and a
 * sentence on `refusal` when Fast DDS would not produce it.
 *
 * Both passes and not one: pass one is the generated type support
 * unchanged, whose header carries the `EncodingAlgorithmFlag` that
 * `fastddsgen` chose for the type, and pass two writes in the byte order
 * the caller asked for behind that same flag. The reasoning is above and
 * has not changed; only the three settings have moved from `sed` to
 * arguments.
 *
 * ## The fill buffer, checked
 *
 * The defect the RTI probe carried — four `--matrix` combinations
 * serialising into one buffer that is never cleared, so each reports the
 * previous one's octets at its alignment gaps — cannot arise here, and
 * that is a property of the code rather than of a run. `raw` is a local
 * of this function and `std::vector<char> raw(CAPACITY)`
 * value-initialises every element, so each combination gets a buffer of
 * `00` whatever the one before it wrote. `SerializedPayload_t vendor` in
 * pass one is likewise per call.
 *
 * Nothing to clear, then, and the zeros the record holds are also what
 * Fast DDS puts on the wire: over RTPS/UDP through a real DataWriter the
 * fill spans are `00`, 30 samples in each encoding version
 * (`tools/wire-capture`). Checked 2026-08-13. */
static bool zd_write(DataRepresentationId_t representation, CdrVersion version,
                     Cdr::Endianness endianness, @TYPE@ &sample,
                     std::vector<unsigned char> &payload, std::string &refusal) {
    payload.clear();
    refusal.clear();

    /* Pass one: the generated type support, unchanged. A case that Fast
       DDS refuses can throw out of this call rather than return false
       — an out-of-range union discriminator does — and an uncaught
       throw here takes the whole probe down with SIGABRT, which the
       pairwise runner counts as an infrastructure failure rather than
       the vendor refusal it is. */
    @TYPE@PubSubType support;
    SerializedPayload_t vendor(CAPACITY);
    try {
        zd_vendor_call("fast dds", "serialising the sample");
        const bool serialised = support.serialize(&sample, vendor, representation);
        zd_vendor_return();
        if (!serialised) {
            refusal = "fast dds refused to serialise the sample";
            return false;
        }
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        zd_vendor_return();
        refusal = std::string("fast cdr: ") + error.what();
        return false;
    }
    if (vendor.length < 4) {
        refusal = "fast dds wrote no encapsulation header";
        return false;
    }
    const EncodingAlgorithmFlag flag =
        static_cast<EncodingAlgorithmFlag>(vendor.data[1] & 0xFE);

    /* Pass two: the same generated `serialize`, in the byte order the
       caller asked for, behind the header pass one reported. */
    std::vector<char> raw(CAPACITY);
    FastBuffer buffer(raw.data(), raw.size());
    Cdr serialiser(buffer, endianness, version);
    if (!serialiser.set_encoding_flag(flag)) {
        refusal = "fast cdr refused the encoding flag";
        return false;
    }
    try {
        serialiser.serialize_encapsulation();
        serialiser << sample;
        serialiser.set_dds_cdr_options({0, 0});
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        refusal = std::string("fast cdr: ") + error.what();
        return false;
    }
    const std::size_t length = serialiser.get_serialized_data_length();
    if (length < 4) {
        refusal = "fast cdr wrote no encapsulation header";
        return false;
    }

    if (Cdr::LITTLE_ENDIANNESS == serialiser.endianness() &&
        (vendor.length != length ||
         std::memcmp(vendor.data, raw.data(), length) != 0)) {
        refusal = "the two passes disagree in little endian: the encoding "
                  "flag read back from the header is wrong";
        return false;
    }

    /* `serialize_encapsulation` wrote the four-byte header. `--emit
       payload` is the payload, so strip exactly that and nothing
       else. */
    payload.assign(raw.begin() + 4, raw.begin() + static_cast<long>(length));
    return true;
}

int main(int argc, char **argv) {
    @TYPE@ sample;

    /* `fill.inc` calls the generated type's own setters — for a union,
       that includes the discriminant setter, which Fast DDS validates
       against the switch's labels and refuses with a
       `BadParamException` when a value has none of them. A refusal
       from filling the sample is the same kind of vendor answer as one
       from serialising it; uncaught, it aborts the process before
       either mode below prints anything. */
    try {
#include "fill.inc"
    } catch (const eprosima::fastcdr::exception::Exception &error) {
        const std::string refusal = std::string("fast cdr: ") + error.what();
        if (argc == 2 && std::strcmp(argv[1], "--matrix") == 0) {
            static const char *const names[2] = { "xcdr1", "xcdr2" };
            static const char *const orders[2] = { "le", "be" };
            for (int v = 0; v < 2; v++) {
                for (int o = 0; o < 2; o++) {
                    std::printf("%s\t%s\t!%s\n", names[v], orders[o], refusal.c_str());
                }
            }
            return 0;
        }
        std::fprintf(stderr, "%s\n", refusal.c_str());
        return 1;
    }

    std::vector<unsigned char> payload;
    std::string refusal;

    if (argc == 2 && std::strcmp(argv[1], "--matrix") == 0) {
        static const char *const names[2] = { "xcdr1", "xcdr2" };
        static const DataRepresentationId_t representations[2] = {
            DataRepresentationId_t::XCDR_DATA_REPRESENTATION,
            DataRepresentationId_t::XCDR2_DATA_REPRESENTATION
        };
        static const CdrVersion versions[2] = { CdrVersion::XCDRv1, CdrVersion::XCDRv2 };
        static const char *const orders[2] = { "le", "be" };
        static const Cdr::Endianness endiannesses[2] = {
            Cdr::LITTLE_ENDIANNESS, Cdr::BIG_ENDIANNESS
        };
        for (int v = 0; v < 2; v++) {
            for (int o = 0; o < 2; o++) {
                /* A refusal is a row. An absent line would leave the
                   caller to decide what the absence meant, and the two
                   meanings — refused and never asked — are the two this
                   tree keeps apart. */
                if (!zd_write(representations[v], versions[v], endiannesses[o],
                              sample, payload, refusal)) {
                    std::printf("%s\t%s\t!%s\n", names[v], orders[o], refusal.c_str());
                    continue;
                }
                std::printf("%s\t%s\t", names[v], orders[o]);
                for (unsigned char byte : payload) std::printf("%02x", byte);
                std::printf("\n");
            }
        }
        return 0;
    }

    if (argc != 3) {
        std::fprintf(stderr, "usage: probe <le|be> <1|2>   or   probe --matrix\n");
        return 2;
    }
    const bool big = std::strcmp(argv[1], "be") == 0;
    const bool second = std::strcmp(argv[2], "2") == 0;
    if (!zd_write(second ? DataRepresentationId_t::XCDR2_DATA_REPRESENTATION
                         : DataRepresentationId_t::XCDR_DATA_REPRESENTATION,
                  second ? CdrVersion::XCDRv2 : CdrVersion::XCDRv1,
                  big ? Cdr::BIG_ENDIANNESS : Cdr::LITTLE_ENDIANNESS,
                  sample, payload, refusal)) {
        std::fprintf(stderr, "%s\n", refusal.c_str());
        return 1;
    }
    if (std::fwrite(payload.data(), 1, payload.size(), stdout) != payload.size()) {
        std::fprintf(stderr, "the bytes could not be written\n");
        return 1;
    }
    return 0;
}
