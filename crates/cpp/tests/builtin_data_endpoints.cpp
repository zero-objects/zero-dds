// DDS-XTypes 1.3 Annex E: the DataWriter and DataReader classes of the four built-in types:
// write, take and read, the instance operations by key, the byte range of write_w_bytes.
#include <dds/dds.hpp>

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <thread>
#include <vector>

#include "zerodds/builtin_data_endpoints.hpp"

static int g_failures = 0;
#define EXPECT(cond, what)                                                     \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::printf("FAIL line %d: %s\n", __LINE__, what);                 \
            ++g_failures;                                                      \
        }                                                                      \
    } while (0)

// Takes the next sample once the match is there: the writer's samples before it are not kept.
template <typename Reader, typename Write, typename Take>
static bool eventually(Reader&, Write write, Take take) {
    for (int attempt = 0; attempt < 100; ++attempt) {
        write();
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
        if (take()) {
            return true;
        }
    }
    return false;
}

int main(int argc, char** argv) {
    const uint32_t domain = argc > 1 ? static_cast<uint32_t>(std::atoi(argv[1])) : 0;
    dds::domain::DomainParticipant participant(domain);
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
        const bool got = eventually(
            reader, [&] { writer.write("hello"); },
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
        const bool got = eventually(
            reader, [&] { writer.write_string_w_key("a", "one", handle); },
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
        const bool got = eventually(
            reader, [&] { writer.write_w_bytes(bytes, 1, 3); },
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
        const bool got = eventually(
            reader, [&] { writer.write_bytes_w_key("k", bytes, 0, 3, handle); },
            [&] { return reader.take_next(key, value, info) == DDS::RETCODE_OK; });
        EXPECT(got && key == "k" && value == bytes, "keyed bytes: written and taken");
    }

    if (g_failures == 0) {
        std::printf("BUILTIN DATA ENDPOINTS OK\n");
    }
    return g_failures == 0 ? 0 : 1;
}
