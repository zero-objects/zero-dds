// A type `zerodds-idlc generate --lang c++ --cpp-mapping native` emitted, through the DDS-PSM-Cxx
// DataWriter<T> and DataReader<T> (dds/pub/Publisher.hpp,
// dds/sub/Subscriber.hpp).
//
// Three things the ByteSeq smoke test cannot show:
//   * DataReader<T>::take() compiles against the generated four-argument
//     decode and decodes in the representation the SampleInfo reports;
//   * the writer and reader carry the generated wire contract, so an
//     @appendable sample is framed as the body encode() writes;
//   * a keyed sample is its own key's instance: two samples of key 5 share an
//     instance handle, key 6 has another;
//   * dispose_instance(sample) of a generated keyed type compiles and reaches
//     the reader as that key's instance going NOT_ALIVE_DISPOSED.
#include "typed_probe.hpp"
#include "dds/dds.hpp"
#include "reliable_pair.hpp"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <set>
#include <thread>
#include <vector>

static int g_failures = 0;
#define EXPECT(cond, what)                                                     \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::printf("FAIL: %s\n", what);                                   \
            ++g_failures;                                                      \
        }                                                                      \
    } while (0)

int main(int argc, char** argv) {
    const int domain = argc > 1 ? std::atoi(argv[1]) : 0;
    dds::domain::DomainParticipant dp(domain);
    dds::topic::Topic<probe::Reading> topic(dp, "GeneratedTypeRoundtrip");
    ReliablePair<probe::Reading> pair(dp, topic);
    dds::pub::DataWriter<probe::Reading>& writer = pair.writer;
    dds::sub::DataReader<probe::Reading>& reader = pair.reader;

    const std::vector<probe::Reading> sent = {{5, 1.5, "five"}, {5, 2.5, "five again"}, {6, 3.5, "six"}};
    for (const auto& s : sent) writer.write(s);

    std::vector<dds::sub::Sample<probe::Reading>> got;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (got.size() < sent.size() && std::chrono::steady_clock::now() < deadline) {
        for (auto& s : reader.take()) {
            if (s.info().valid()) got.push_back(s);
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    {
        // DDS-PSM-Cxx 1.0 §7.12: `swap` as a member and in the type's namespace.
        probe::Reading a = sent[0];
        probe::Reading b = sent[2];
        swap(a, b);
        EXPECT(a.id == sent[2].id && b.id == sent[0].id && a.label == sent[2].label,
               "swap in the namespace exchanges the members");
        a.swap(b);
        EXPECT(a.id == sent[0].id && b.label == sent[2].label, "the member swap exchanges them back");
    }
    EXPECT(got.size() == sent.size(), "every sample arrived");
    std::set<uint64_t> five, six;
    for (std::size_t i = 0; i < got.size() && i < sent.size(); ++i) {
        const auto& d = got[i].data();
        EXPECT(d.id == sent[i].id && d.value == sent[i].value && d.label == sent[i].label,
               "a sample came back with other values");
        (d.id == 5 ? five : six).insert(got[i].info().instance_handle().value());
    }
    EXPECT(five.size() == 1, "two samples of key 5 are one instance");
    EXPECT(six.size() == 1 && five != six, "key 6 is another instance");

    writer.dispose_instance(writer.lookup_instance(sent[0]));
    bool disposed = false;
    const auto dispose_deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (!disposed && std::chrono::steady_clock::now() < dispose_deadline) {
        for (auto& s : reader.take()) {
            if (s.info().state().instance_state() ==
                    dds::sub::status::InstanceState::not_alive_disposed() &&
                five.count(s.info().instance_handle().value()) == 1) {
                disposed = true;
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    EXPECT(disposed, "the dispose of key 5 reached the reader as key 5's instance");

    // Instances by handle (DDS-PSM-Cxx 1.0 §7.9): register, look up, write to
    // the instance, key_value, a handle that is not the sample's.
    const dds::core::InstanceHandle h6 = writer.register_instance(sent[2]);
    EXPECT(!h6.is_nil(), "register_instance gives a handle");
    EXPECT(writer.lookup_instance(sent[2]) == h6, "lookup_instance finds the registered instance");
    probe::Reading key_out;
    writer.key_value(key_out, h6);
    EXPECT(key_out.id == sent[2].id, "key_value gives the key of the instance");
    writer.write(sent[2], h6);
    bool mismatch = false;
    try {
        writer.write(sent[0], h6);
    } catch (const dds::core::InvalidArgumentError&) {
        mismatch = true;
    }
    EXPECT(mismatch, "a handle that is not the sample's instance is refused");

    bool keyed = false;
    const auto key_deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
    while (!keyed && std::chrono::steady_clock::now() < key_deadline) {
        for (auto& s : reader.take()) {
            if (s.info().valid() && s.data().id == sent[2].id) {
                probe::Reading from_reader;
                reader.key_value(from_reader, s.info().instance_handle());
                keyed = from_reader.id == sent[2].id;
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    EXPECT(keyed, "the reader's key_value gives the key of a sample it returned");
    writer.unregister_instance(h6);

    // A content filter reads fields of the generated type (the reader knows the type's
    // CompleteTypeObject), alone and with an instance handle.
    {
        writer.write(probe::Reading{7, 10.0, "low"});
        writer.write(probe::Reading{8, 90.0, "high"});
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        dds::sub::Query query(reader, "value > %0", std::vector<std::string>{"50"});
        auto selected = reader.select().content(query).read();
        bool only_high = selected.length() > 0;
        for (auto it = selected.begin(); it != selected.end(); ++it) {
            if (it->info().valid() && it->data().value <= 50.0) only_high = false;
        }
        EXPECT(only_high, "a query on `value` returns only the samples above the parameter");
        const dds::core::InstanceHandle low = writer.lookup_instance(probe::Reading{7, 10.0, "low"});
        EXPECT(reader.select().instance(low).content(query).read().length() == 0,
               "a query together with instance(): the instance's sample does not pass");
    }

    if (g_failures != 0) {
        std::printf("GENERATED TYPE ROUNDTRIP FAILED: %d\n", g_failures);
        return 1;
    }
    std::printf("GENERATED TYPE ROUNDTRIP OK\n");
    return 0;
}
