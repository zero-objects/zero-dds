// DDS-XTypes 1.3 Annex E: the DataWriter and DataReader classes of the four built-in types:
// write, take and read, the instance operations by key, the byte range of write_w_bytes.
#include <dds/dds.hpp>

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#include "zerodds/builtin_data_endpoints.hpp"
#include "test_support.hpp"

static int g_failures = 0;
#define EXPECT(cond, what)                                                     \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::printf("FAIL line %d: %s\n", __LINE__, what);                 \
            ++g_failures;                                                      \
        }                                                                      \
    } while (0)

// Waits on one condition of the C API in a WaitSet (DDS 1.4 §2.2.2.1.6); true when it triggered
// within ten seconds.
static bool wait_on(void* condition) {
    zerodds_ZeroDdsWaitSet* waitset = zerodds_waitset_create();
    zerodds_waitset_attach_condition(waitset, condition);
    void* active[1] = {nullptr};
    uintptr_t count = 0;
    const int rc = zerodds_waitset_wait(waitset, active, 1, &count, 10, 0);
    zerodds_waitset_detach_condition(waitset, condition);
    zerodds_waitset_destroy(waitset);
    return rc == 0 && count > 0;
}

// Writes once the writer has matched the reader (PUBLICATION_MATCHED on its StatusCondition,
// DDS 1.4 §2.2.4.1; the writer's samples before the match are not kept) and takes the sample
// once the reader holds one (a ReadCondition of any state, §2.2.2.5.8).
template <typename Writer, typename Reader, typename Write, typename Take>
static bool written_and_taken(Writer& writer, Reader& reader, Write write, Take take) {
    zerodds_ZeroDdsStatusCondition* matched = zerodds_entity_get_statuscondition(writer.native_handle());
    zerodds_statuscondition_set_enabled_statuses(matched, 1u << 13);
    const bool match = wait_on(matched);
    zerodds_statuscondition_destroy(matched);
    if (!match) {
        return false;
    }
    write();
    zerodds_ZeroDdsReadCondition* data = zerodds_dr_create_readcondition(reader.native_handle(), 3u, 3u, 7u);
    const bool arrived = wait_on(data);
    zerodds_dr_delete_readcondition(reader.native_handle(), data);
    return arrived && take();
}

int main(int argc, char** argv) {
    dds::domain::DomainParticipant participant(test_domain(argc, argv));
    dds::pub::Publisher publisher(participant);
    dds::sub::Subscriber subscriber(participant);

    // DDS::String
    {
        DDS::StringDataWriter writer(participant, publisher, "CppString");
        DDS::StringDataReader reader(participant, subscriber, "CppString");
        EXPECT(writer.is_valid() && reader.is_valid(), "string endpoints");
        std::string value;
        DDS::BuiltinSampleInfo info;
        EXPECT(reader.take_next(value, info) == DDS::RETCODE_NO_DATA, "string: no data yet");
        const bool got = written_and_taken(
            writer, reader, [&] { writer.write("hello"); },
            [&] { return reader.take_next(value, info) == DDS::RETCODE_OK; });
        EXPECT(got && value == "hello" && info.valid_data, "string: written and taken");
        EXPECT(writer.write_w_timestamp("stamped", 1, 2) == DDS::RETCODE_OK, "string: timestamp");
        EXPECT(writer.write_w_timestamp("bad", 1, 2000000000u) == DDS::RETCODE_BAD_PARAMETER,
               "string: a nanosecond count of 10^9 or more is no time");
    }

    // DDS::KeyedString
    {
        DDS::KeyedStringDataWriter writer(participant, publisher, "CppKeyedString");
        DDS::KeyedStringDataReader reader(participant, subscriber, "CppKeyedString");
        EXPECT(writer.is_valid() && reader.is_valid(), "keyed string endpoints");
        DDS::InstanceHandle_t handle = 0;
        EXPECT(writer.register_instance_w_key("a", handle) == DDS::RETCODE_OK && handle != 0,
               "keyed string: register");
        EXPECT(writer.lookup_instance_w_key("a") == handle, "keyed string: lookup");
        EXPECT(writer.lookup_instance_w_key("b") == 0, "keyed string: b is not registered");
        std::string key_back;
        EXPECT(writer.get_key_value_w_key(key_back, handle) == DDS::RETCODE_OK && key_back == "a",
               "keyed string: key of the handle");
        EXPECT(writer.write_string_w_key("b", "v", handle) != DDS::RETCODE_OK,
               "keyed string: the handle is a's, the key is b");
        std::string key, value;
        DDS::BuiltinSampleInfo info;
        const bool got = written_and_taken(
            writer, reader, [&] { writer.write_string_w_key("a", "one", handle); },
            [&] { return reader.take_next(key, value, info) == DDS::RETCODE_OK; });
        EXPECT(got && key == "a" && value == "one", "keyed string: written and taken");
        EXPECT(writer.dispose_w_key("a") == DDS::RETCODE_OK, "keyed string: dispose");
        EXPECT(writer.unregister_instance_w_key("a") == DDS::RETCODE_OK, "keyed string: unregister");
    }

    // DDS::Bytes
    {
        DDS::BytesDataWriter writer(participant, publisher, "CppBytes");
        DDS::BytesDataReader reader(participant, subscriber, "CppBytes");
        EXPECT(writer.is_valid() && reader.is_valid(), "bytes endpoints");
        const std::vector<uint8_t> bytes = {9, 1, 2, 3, 9};
        std::vector<uint8_t> value;
        DDS::BuiltinSampleInfo info;
        const bool got = written_and_taken(
            writer, reader, [&] { writer.write_w_bytes(bytes, 1, 3); },
            [&] { return reader.take_next(value, info) == DDS::RETCODE_OK; });
        EXPECT(got && value == std::vector<uint8_t>({1, 2, 3}), "bytes: offset 1, length 3");
        EXPECT(writer.write_w_bytes(bytes, 3, 5) != DDS::RETCODE_OK,
               "bytes: a range outside the array");
    }

    // DDS::KeyedBytes
    {
        DDS::KeyedBytesDataWriter writer(participant, publisher, "CppKeyedBytes");
        DDS::KeyedBytesDataReader reader(participant, subscriber, "CppKeyedBytes");
        EXPECT(writer.is_valid() && reader.is_valid(), "keyed bytes endpoints");
        DDS::InstanceHandle_t handle = 0;
        EXPECT(writer.register_instance_w_key("k", handle) == DDS::RETCODE_OK && handle != 0,
               "keyed bytes: register");
        const std::vector<uint8_t> bytes = {7, 8, 9};
        std::string key;
        std::vector<uint8_t> value;
        DDS::BuiltinSampleInfo info;
        const bool got = written_and_taken(
            writer, reader, [&] { writer.write_bytes_w_key("k", bytes, 0, 3, handle); },
            [&] { return reader.take_next(key, value, info) == DDS::RETCODE_OK; });
        EXPECT(got && key == "k" && value == bytes, "keyed bytes: written and taken");
    }

    if (g_failures == 0) {
        std::printf("BUILTIN DATA ENDPOINTS OK\n");
    }
    return g_failures == 0 ? 0 : 1;
}
