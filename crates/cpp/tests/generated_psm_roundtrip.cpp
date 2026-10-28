// A type `zerodds-idlc generate --lang c++ --cpp-mapping psm` emitted: the DDS-PSM-Cxx 1.0
// §8.1 classes, through the same codec as the native ones.
//
//   * the accessors of §7.4.6, `swap`, `dds::core::optional`, `dds::core::array`, `std::map`,
//     `std::wstring`, `wchar_t` and `long double` hold what was put in;
//   * `topic_type_support<PSM>::encode` writes the octets of the native codec, and `decode`
//     reads them back to an equal sample;
//   * a DataWriter<PSM> and a DataReader<PSM> carry a sample over the C API.
#include "psm_probe.hpp"
#include "dds/dds.hpp"
#include "reliable_pair.hpp"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <functional>
#include <map>
#include <string>
#include <thread>
#include <type_traits>
#include <utility>
#include <vector>

static int g_failures = 0;
#define EXPECT(cond, what)                                                     \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::printf("FAIL: %s\n", what);                                   \
            ++g_failures;                                                      \
        }                                                                      \
    } while (0)

// DDS-PSM-Cxx 1.0 Table 7.1: the C++ type of every DDS type, as the accessors of the generated
// class give it.
template <typename T, typename Expected>
struct is_member_type : std::is_same<typename std::decay<T>::type, Expected> {};
static void check_table_7_1(psmprobe::Primitives& p) {
    static_assert(is_member_type<decltype(p.b()), bool>::value, "Boolean is bool");
    static_assert(is_member_type<decltype(p.c()), char>::value, "Char8 is char");
    static_assert(is_member_type<decltype(p.w()), wchar_t>::value, "Char32 is wchar_t");
    static_assert(is_member_type<decltype(p.o()), uint8_t>::value, "Byte is uint8_t");
    static_assert(is_member_type<decltype(p.s()), int16_t>::value, "Int16 is int16_t");
    static_assert(is_member_type<decltype(p.us()), uint16_t>::value, "UInt16 is uint16_t");
    static_assert(is_member_type<decltype(p.l()), int32_t>::value, "Int32 is int32_t");
    static_assert(is_member_type<decltype(p.ul()), uint32_t>::value, "UInt32 is uint32_t");
    static_assert(is_member_type<decltype(p.ll()), int64_t>::value, "Int64 is int64_t");
    static_assert(is_member_type<decltype(p.ull()), uint64_t>::value, "UInt64 is uint64_t");
    static_assert(is_member_type<decltype(p.f()), float>::value, "Float32 is float");
    static_assert(is_member_type<decltype(p.d()), double>::value, "Float64 is double");
    static_assert(is_member_type<decltype(p.ld()), long double>::value, "Float128 is long double");
    static_assert(is_member_type<decltype(p.str()), std::string>::value, "string<Char8> is std::string");
    static_assert(is_member_type<decltype(p.wstr()), std::wstring>::value,
                  "string<Char32> is std::wstring");
    static_assert(is_member_type<decltype(p.seq()), std::vector<int32_t> >::value,
                  "sequence<T> is std::vector<T>");
    static_assert(is_member_type<decltype(p.bounded()), std::vector<int32_t> >::value,
                  "a bounded sequence is the same type as an unbounded one");
    static_assert(is_member_type<decltype(p.m()), std::map<int32_t, double> >::value,
                  "map<K, V> is std::map<K, V>");
    static_assert(is_member_type<decltype(p.arr()), dds::core::array<int32_t, 3> >::value,
                  "T[N] is dds::core::array<T, N>");
    // The fixed-size integer types are the global ones of <stdint.h>, not names in std.
    static_assert(std::is_same<::int32_t, int32_t>::value && std::is_same<::uint8_t, uint8_t>::value,
                  "the integer types are in the global namespace");
}

static psmprobe::Reading make_reading(int32_t id) {
    psmprobe::Reading r;
    r.id(id);
    r.value(2.5);
    r.label("label");
    r.wlabel(L"wide äö");
    r.initial(L'x');
    r.precise(1.25L);
    r.shade(psmprobe::Colour::GREEN);
    r.nested(psmprobe::Inner(7, "inner"));
    r.history().push_back(psmprobe::Inner(1, "one"));
    r.history().push_back(psmprobe::Inner(2, "two"));
    r.counts() = {3, 4, 5};
    r.cells()[1][2] = 42;
    r.tags()["b"] = 2;
    r.tags()["a"] = 1;
    r.maybe(11);
    r.names().push_back(L"n1");
    return r;
}

// IDL4 to C++ 1.0 §7.2.4.3.2 (DDS-PSM-Cxx 1.0 §7.4.4 maps unions as IDL2C++11 §6.13.2 does):
// the shape of the union class, from the example of the specification.
static void check_union_mapping() {
    using U = psmprobe::SpecUnion;
    using V = psmprobe::ImplicitUnion;
    // Built-in members: the constant accessor by value, the other by reference; a structure:
    // by constant reference and by reference.
    static_assert(std::is_same<decltype(std::declval<const U&>().a_long()), int32_t>::value,
                  "a built-in member: the constant accessor returns by value");
    static_assert(std::is_same<decltype(std::declval<U&>().a_long()), int32_t&>::value,
                  "and the other by reference");
    static_assert(std::is_same<decltype(std::declval<const U&>().a_struct()), const psmprobe::Piece&>::value,
                  "a structure member: constant reference");
    static_assert(std::is_same<decltype(std::declval<U&>().a_struct()), psmprobe::Piece&>::value,
                  "and reference");
    static_assert(std::is_same<decltype(std::declval<const U&>()._d()), uint8_t>::value, "_d() const");
    static_assert(std::is_nothrow_move_constructible<U>::value || std::is_move_constructible<U>::value,
                  "a move constructor");
    static_assert(std::is_move_assignable<U>::value, "a move assignment");
    // The modifier with a discriminator exists for a member under two labels.
    static_assert(std::is_same<decltype(std::declval<U&>().a_short(int16_t(), uint8_t())), void>::value,
                  "a modifier with a discriminator for a member under two labels");

    // The default constructor: a union with a default case starts on the default branch.
    U with_default;
    EXPECT(with_default._d() != 1 && with_default._d() != 2 && with_default._d() != 3 &&
               with_default._d() != 4,
           "the default constructor selects the default branch of a union with a default case");
    // Without one, on the first label.
    V implicit;
    EXPECT(implicit._d() == 1, "the default constructor selects the first label");
    // A modifier selects the case of its member.
    with_default.a_long(7);
    EXPECT(with_default._d() == 1 && with_default.a_long() == 7, "a modifier selects its case");
    with_default.a_short(5);
    EXPECT(with_default._d() == 2 && with_default.a_short() == 5, "a member under two labels: the first");
    with_default.a_short(6, 3);
    EXPECT(with_default._d() == 3 && with_default.a_short() == 6,
           "the modifier with a discriminator selects the other label");
    psmprobe::Piece piece;
    piece.x(9);
    with_default.a_struct(piece);
    EXPECT(with_default._d() == 4 && with_default.a_struct().x() == 9, "a structure member by constant reference");
    psmprobe::Piece moved;
    moved.x(10);
    with_default.a_struct(std::move(moved));
    EXPECT(with_default.a_struct().x() == 10, "and by rvalue reference");
    with_default.a_byte_default(200);
    EXPECT(with_default._d() != 1 && with_default._d() != 2 && with_default._d() != 3 &&
               with_default._d() != 4 && with_default.a_byte_default() == 200,
           "the default branch selects a value no label names");
    // The discriminator by itself.
    with_default._d(2);
    EXPECT(with_default._d() == 2, "_d(value) sets the discriminator");

    // An implicit default member: _default() sets a legal value no label names.
    implicit.a_short(4);
    implicit._default();
    EXPECT(implicit._d() != 1 && implicit._d() != 2 && implicit._d() != 3, "_default() selects the implicit default");

    // Copy, move, swap, equality.
    U a;
    a.a_long(1);
    U b = a;
    EXPECT(a == b, "copy");
    U c = std::move(b);
    EXPECT(c == a, "move");
    U d;
    d.a_long(2);
    swap(c, d);
    EXPECT(c.a_long() == 2 && d.a_long() == 1, "swap in the namespace");

    // Through the codec.
    psmprobe::UnionHolder h;
    h.key(3);
    U with;
    with.a_short(11, 3);
    h.with_default(with);
    V without;
    without.a_long(12);
    h.without_default(without);
    const auto octets = dds::topic::topic_type_support<psmprobe::UnionHolder>::encode(h);
    const psmprobe::UnionHolder back = dds::topic::topic_type_support<psmprobe::UnionHolder>::decode(
        octets.data(), octets.size(), zerodds::wire::Encoding::PlainCdr2, false);
    EXPECT(back.key() == 3, "the key of the holder survives");
    EXPECT(back.with_default()._d() == 3, "the discriminator of the union with a default case survives");
    EXPECT(back.with_default().a_short() == 11, "the member under the second label survives");
    EXPECT(back.without_default()._d() == 1, "the discriminator of the implicit-default union survives");
    EXPECT(back.without_default().a_long() == 12, "its member survives");
    EXPECT(back.with_default() == h.with_default(), "the union with a default case compares equal");
    EXPECT(back.without_default() == h.without_default(), "the implicit-default union compares equal");
    EXPECT(back == h, "the holder compares equal");
}

int main(int argc, char** argv) {
    using Support = dds::topic::topic_type_support<psmprobe::Reading>;

    check_union_mapping();

    // The class, without a network.
    psmprobe::Reading r = make_reading(5);
    EXPECT(r.id() == 5 && r.value() == 2.5 && r.label() == "label", "accessors");
    EXPECT(r.maybe().is_set() && r.maybe().get() == 11, "an @optional member is dds::core::optional");
    EXPECT(!r.maybe_inner().is_set(), "an absent @optional member");
    EXPECT(r.cells()[1][2] == 42 && r.tags().size() == 2, "dds::core::array and std::map");
    // DDS-XTypes 1.3 §7.5.1.3.4: "When a map has keys of a string type, the Compare function
    // shall operate on the character contents of the strings; it shall not operate on the
    // strings' pointer values (as std::less does)." The key is a std::string, and
    // `std::less<std::string>` compares the characters: an equal string of another object is
    // the same key, and the entries are in the order of the characters, not of the insertion.
    static_assert(std::is_same<std::decay<decltype(r.tags())>::type::key_compare,
                               std::less<std::string> >::value,
                  "the Compare of a string-keyed map is std::less<std::string>");
    {
        std::string other_a = std::string("a") + std::string();
        EXPECT(r.tags().find(other_a) != r.tags().end() && r.tags().find(other_a)->second == 1,
               "a string key is found by its characters");
        EXPECT(r.tags().begin()->first == "a" && r.tags().rbegin()->first == "b",
               "the entries of a string-keyed map are in the order of the characters");
    }
    psmprobe::Reading copy = r;
    EXPECT(copy == r, "equality");
    psmprobe::Reading other = make_reading(6);
    swap(copy, other);
    EXPECT(copy.id() == 6 && other.id() == 5, "swap in the namespace");
    // §7.12: a generated class moves (idl2cpp11), and its member swap exchanges the state.
    static_assert(std::is_move_constructible<psmprobe::Reading>::value,
                  "a generated class is move constructible");
    psmprobe::Reading moved_to(std::move(other));
    EXPECT(moved_to.id() == 5 && moved_to.label() == "label", "a generated class moves");
    moved_to.swap(copy);
    EXPECT(moved_to.id() == 6 && copy.id() == 5, "the member swap exchanges the state");

    psmprobe::Primitives primitives;
    check_table_7_1(primitives);
    primitives.b(true);
    primitives.w(L'w');
    primitives.o(200);
    primitives.us(60000);
    primitives.ull(0xFFFFFFFFFFFFFFFFull);
    primitives.f(1.5f);
    primitives.ld(2.5L);
    primitives.wstr(L"wide");
    primitives.seq() = {1, 2, 3};
    primitives.bounded() = {4, 5};
    primitives.m()[7] = 0.5;
    primitives.arr()[2] = 9;
    const std::vector<uint8_t> primitive_octets = dds::topic::topic_type_support<psmprobe::Primitives>::encode(primitives);
    const psmprobe::Primitives primitives_back = dds::topic::topic_type_support<psmprobe::Primitives>::decode(
        primitive_octets.data(), primitive_octets.size(), zerodds::wire::Encoding::PlainCdr2, false);
    EXPECT(primitives_back == primitives, "every type of Table 7.1 survives the codec");

    // The octets are the codec's.
    const std::vector<uint8_t> octets = Support::encode(r);
    using Native = dds::topic::topic_type_support<::zd_native::psmprobe::Reading>;
    EXPECT(octets == Native::encode(zerodds::psm::Conv<psmprobe::Reading>::to_native(r)),
           "encode writes the native codec's octets");
    const psmprobe::Reading back =
        Support::decode(octets.data(), octets.size(), zerodds::wire::Encoding::PlainCdr2, false);
    EXPECT(back == r, "decode reads back an equal sample");
    EXPECT(back.precise() == 1.25L, "long double through binary128");
    EXPECT(back.wlabel() == r.wlabel(), "wstring through UTF-8");
    EXPECT(back.maybe().is_set() && !back.maybe_inner().is_set(), "optional presence");

    psmprobe::Choice c;
    c.text("hello");
    EXPECT(c._d() == 2, "a union setter selects its case");
    psmprobe::Holder h;
    h.key(9);
    h.pick(c);
    const auto holder_octets = dds::topic::topic_type_support<psmprobe::Holder>::encode(h);
    const psmprobe::Holder holder_back = dds::topic::topic_type_support<psmprobe::Holder>::decode(
        holder_octets.data(), holder_octets.size(), zerodds::wire::Encoding::PlainCdr2, false);
    EXPECT(holder_back.key() == 9 && holder_back.pick()._d() == 2 && holder_back.pick().text() == "hello",
           "a union member survives the codec");

    // Over the C API.
    const int domain = argc > 1 ? std::atoi(argv[1]) : 0;
    dds::domain::DomainParticipant dp(domain);
    dds::topic::Topic<psmprobe::Reading> topic(dp, "GeneratedPsmRoundtrip");
    ReliablePair<psmprobe::Reading> pair(dp, topic);
    dds::pub::DataWriter<psmprobe::Reading>& writer = pair.writer;
    dds::sub::DataReader<psmprobe::Reading>& reader = pair.reader;

    writer.write(r);
    std::vector<dds::sub::Sample<psmprobe::Reading>> got;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (got.empty() && std::chrono::steady_clock::now() < deadline) {
        for (auto& s : reader.take()) {
            if (s.info().valid()) got.push_back(s);
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    EXPECT(got.size() == 1, "the sample arrived");
    if (!got.empty()) {
        EXPECT(got[0].data() == r, "the sample came back equal");
    }

    // The application of DDS-PSM-Cxx 1.0 §7.13.1 and §7.13.2 over the type of §8.2: a writer with
    // QoS composed from policies, a reader with the same QoS, read into a vector of samples and the
    // selector form.
    {
        using namespace dds;
        using psmprobe::RadarTrack;
        namespace policy = dds::core::policy;
        domain::DomainParticipant dp7(static_cast<uint32_t>(domain));

        pub::qos::PublisherQos pqos;
        pqos << policy::Partition("Tracks");
        pub::Publisher publisher7(dp7, pqos);

        topic::qos::TopicQos tqos;
        tqos << policy::Reliability::Reliable() << policy::Durability::Transient()
             << policy::History::KeepLast(10) << policy::TransportPriority(14);
        topic::Topic<RadarTrack> topic7(dp7, "TrackTopic", tqos);
        pub::qos::DataWriterQos dwqos(tqos);
        pub::DataWriter<RadarTrack> dw7(publisher7, topic7, dwqos);

        sub::qos::SubscriberQos sqos;
        sqos << policy::Partition("Tracks");
        sub::Subscriber subscriber7(dp7, sqos);
        sub::qos::DataReaderQos drqos(tqos);
        sub::DataReader<RadarTrack> dr7(subscriber7, topic7, drqos);
        dw7->wait_for_matched(1, dds::core::Duration(5, 0));
        dr7->wait_for_matched(1, dds::core::Duration(5, 0));

        RadarTrack track("alpha", 100, 200, 5, std::make_shared<psmprobe::plot_t>(psmprobe::plot_t{1, 2, 3}));
        dw7.write(track);
        dw7 << track;

        const unsigned MY_MAX_LEN = 4;
        std::vector<dds::sub::Sample<RadarTrack> > samples7(MY_MAX_LEN);
        unsigned got7 = 0;
        const auto until7 = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (got7 < 2 && std::chrono::steady_clock::now() < until7) {
            got7 = dr7.read(samples7.begin(), MY_MAX_LEN);
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
        }
        EXPECT(got7 >= 2, "the C++03 example reads the samples into a vector");
        if (got7 >= 1) {
            EXPECT(samples7[0].data().id() == "alpha" && samples7[0].data().x() == 100 &&
                       samples7[0].data().y() == 200 && samples7[0].data().z().is_set() &&
                       samples7[0].data().z().get() == 5 && samples7[0].data().plot() &&
                       samples7[0].data().plot()->size() == 3,
                   "the sample has its attributes, the optional one and the shared plot");
        }
        auto taken7 = dr7.select().max_samples(100).state(dds::sub::status::DataState::any()).take();
        size_t count7 = 0;
        for (auto& s7 : taken7) {
            if (s7.info().valid()) {
                EXPECT(s7.data().id() == "alpha", "the C++11 example takes the samples with a range-for loop");
                ++count7;
            }
        }
        EXPECT(count7 >= 2, "and finds them");
    }

    if (g_failures != 0) {
        std::printf("GENERATED PSM ROUNDTRIP FAILED: %d\n", g_failures);
        return 1;
    }
    std::printf("GENERATED PSM ROUNDTRIP OK\n");
    return 0;
}
