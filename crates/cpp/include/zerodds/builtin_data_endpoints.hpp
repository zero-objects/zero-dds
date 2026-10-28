// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// zerodds/builtin_data_endpoints.hpp — the DataWriter and DataReader interfaces of
// DDS-XTypes 1.3 Annex E over the four built-in types: `StringDataWriter`,
// `KeyedStringDataWriter`, `BytesDataWriter`, `KeyedBytesDataWriter` and the four readers.
// Each is "an instantiation of FooDataWriter / FooDataReader defined by the DDS
// specification" for `DDS::String`, `DDS::KeyedString`, `DDS::Bytes` or `DDS::KeyedBytes`, plus
// the operations Annex E adds (`write_string_w_key`, `write_w_bytes`, `register_instance_w_key`,
// and so on).
//
//     DDS::KeyedStringDataWriter writer(participant, publisher, "Chat");
//     writer.write_string_w_key("alice", "hello");
//
// A writer or reader creates its topic of the built-in type under the name it is given (the
// type is `DDS::String` etc., Annex E) and deletes it with itself.

#ifndef ZERODDS_BUILTIN_DATA_ENDPOINTS_HPP
#define ZERODDS_BUILTIN_DATA_ENDPOINTS_HPP

#include <stdint.h>

#include <string>
#include <vector>

#include "dds/domain/DomainParticipant.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/types.hpp"
#include "zerodds.h"
#include "zerodds/builtin_type_support.hpp"

namespace DDS {

/// The handle of an instance (DDS 1.4 §2.2.2.4.2.5); 0 is `HANDLE_NIL`.
typedef uint64_t InstanceHandle_t;

/// The `SampleInfo` of a sample of a built-in type, as the C ABI reports it.
struct BuiltinSampleInfo {
    uint32_t sample_state;
    uint32_t view_state;
    uint32_t instance_state;
    int32_t disposed_generation_count;
    int32_t no_writers_generation_count;
    int32_t sample_rank;
    /// False for a sample that carries no data (a dispose or an unregister).
    bool valid_data;
};

namespace detail {

/// What a status of the C ABI is as a `ReturnCode_t`.
inline ReturnCode_t builtin_return_code(int status) {
    switch (status) {
        case 0:
            return RETCODE_OK;
        case -2:  // a null handle
        case -3:  // a name or a text that is none
        case -6:
            return RETCODE_BAD_PARAMETER;
        case -4:
            return RETCODE_TIMEOUT;
        case -5:
            return RETCODE_PRECONDITION_NOT_MET;
        case -7:
            return RETCODE_NO_DATA;
        case -8:
            return RETCODE_OUT_OF_RESOURCES;
        case -9:
            return RETCODE_NOT_ENABLED;
        case -12:
            return RETCODE_ALREADY_DELETED;
        case -13:
            return RETCODE_UNSUPPORTED;
        default:
            return RETCODE_ERROR;
    }
}

inline BuiltinSampleInfo builtin_sample_info(const zerodds_ZeroDdsSampleInfo& info) {
    BuiltinSampleInfo out;
    out.sample_state = info.sample_state;
    out.view_state = info.view_state;
    out.instance_state = info.instance_state;
    out.disposed_generation_count = info.disposed_generation_count;
    out.no_writers_generation_count = info.no_writers_generation_count;
    out.sample_rank = info.sample_rank;
    out.valid_data = info.valid_data;
    return out;
}

/// The topic of a built-in type and the writer over it; both are released with the object.
template <int Kind>
class BuiltinWriterBase {
public:
    BuiltinWriterBase(dds::domain::DomainParticipant& participant, dds::pub::Publisher& publisher,
                      const std::string& topic_name)
        : participant_(participant.delegate()->native_handle()),
          publisher_(publisher.delegate()->native_handle()),
          topic_(zerodds_dp_create_builtin_topic(participant_, topic_name.c_str(), Kind, nullptr)),
          writer_(topic_ != nullptr
                      ? zerodds_pub_create_builtin_datawriter(publisher_, topic_, nullptr)
                      : nullptr) {}

    ~BuiltinWriterBase() {
        if (writer_ != nullptr) {
            zerodds_pub_delete_datawriter(publisher_, writer_);
        }
        if (topic_ != nullptr) {
            zerodds_dp_delete_topic(participant_, topic_);
        }
    }

    BuiltinWriterBase(const BuiltinWriterBase&) = delete;
    BuiltinWriterBase& operator=(const BuiltinWriterBase&) = delete;

    /// False when the topic or the writer could not be created.
    bool is_valid() const { return writer_ != nullptr; }

    /// The writer of the C ABI.
    zerodds_ZeroDdsDataWriter* native_handle() const { return writer_; }

protected:
    zerodds_ZeroDdsDomainParticipant* participant_;
    zerodds_ZeroDdsPublisher* publisher_;
    zerodds_ZeroDdsTopic* topic_;
    zerodds_ZeroDdsDataWriter* writer_;
};

template <int Kind>
class BuiltinReaderBase {
public:
    BuiltinReaderBase(dds::domain::DomainParticipant& participant, dds::sub::Subscriber& subscriber,
                      const std::string& topic_name)
        : participant_(participant.delegate()->native_handle()),
          subscriber_(subscriber.delegate()->native_handle()),
          topic_(zerodds_dp_create_builtin_topic(participant_, topic_name.c_str(), Kind, nullptr)),
          reader_(topic_ != nullptr
                      ? zerodds_sub_create_builtin_datareader(subscriber_, topic_, nullptr)
                      : nullptr) {}

    ~BuiltinReaderBase() {
        if (reader_ != nullptr) {
            zerodds_sub_delete_datareader(subscriber_, reader_);
        }
        if (topic_ != nullptr) {
            zerodds_dp_delete_topic(participant_, topic_);
        }
    }

    BuiltinReaderBase(const BuiltinReaderBase&) = delete;
    BuiltinReaderBase& operator=(const BuiltinReaderBase&) = delete;

    /// False when the topic or the reader could not be created.
    bool is_valid() const { return reader_ != nullptr; }

    /// The reader of the C ABI.
    zerodds_ZeroDdsDataReader* native_handle() const { return reader_; }

protected:
    zerodds_ZeroDdsDomainParticipant* participant_;
    zerodds_ZeroDdsSubscriber* subscriber_;
    zerodds_ZeroDdsTopic* topic_;
    zerodds_ZeroDdsDataReader* reader_;
};

/// A string the C ABI gave out, as `std::string`; released.
inline std::string builtin_take_string(char* text) {
    std::string out = text != nullptr ? std::string(text) : std::string();
    zerodds_builtin_string_free(text);
    return out;
}

/// Octets the C ABI gave out, as a vector; released.
inline std::vector<uint8_t> builtin_take_bytes(uint8_t* bytes, uintptr_t length) {
    std::vector<uint8_t> out;
    if (bytes != nullptr) {
        out.assign(bytes, bytes + length);
    }
    zerodds_builtin_bytes_free(bytes, length);
    return out;
}

}  // namespace detail

/// `StringDataWriter` (Annex E).
class StringDataWriter : public detail::BuiltinWriterBase<0> {
public:
    using BuiltinWriterBase::BuiltinWriterBase;

    /// `write(instance_data)`.
    ReturnCode_t write(const std::string& value) {
        return detail::builtin_return_code(zerodds_string_writer_write(writer_, value.c_str()));
    }
    /// `write_w_timestamp(instance_data, timestamp)`.
    ReturnCode_t write_w_timestamp(const std::string& value, int32_t sec, uint32_t nanosec) {
        return detail::builtin_return_code(
            zerodds_string_writer_write_w_timestamp(writer_, value.c_str(), sec, nanosec));
    }
};

/// `KeyedStringDataWriter` (Annex E): the instance operations name the instance by its key.
class KeyedStringDataWriter : public detail::BuiltinWriterBase<1> {
public:
    using BuiltinWriterBase::BuiltinWriterBase;

    ReturnCode_t register_instance_w_key(const std::string& key, InstanceHandle_t& handle) {
        return detail::builtin_return_code(zerodds_keyed_string_writer_register_instance_w_key(
            writer_, key.c_str(), &handle));
    }
    ReturnCode_t register_instance_w_key_w_timestamp(const std::string& key, int32_t sec,
                                                     uint32_t nanosec, InstanceHandle_t& handle) {
        return detail::builtin_return_code(
            zerodds_keyed_string_writer_register_instance_w_key_w_timestamp(
                writer_, key.c_str(), sec, nanosec, &handle));
    }
    ReturnCode_t unregister_instance_w_key(const std::string& key) {
        return detail::builtin_return_code(
            zerodds_keyed_string_writer_unregister_instance_w_key(writer_, key.c_str()));
    }
    ReturnCode_t unregister_instance_w_key_w_timestamp(const std::string& key, int32_t sec,
                                                       uint32_t nanosec) {
        return detail::builtin_return_code(
            zerodds_keyed_string_writer_unregister_instance_w_key_w_timestamp(
                writer_, key.c_str(), sec, nanosec));
    }
    /// `write_string_w_key(key, str, handle)`; `handle` is 0 or the handle of the key.
    ReturnCode_t write_string_w_key(const std::string& key, const std::string& value,
                                    InstanceHandle_t handle = 0) {
        return detail::builtin_return_code(zerodds_keyed_string_writer_write_string_w_key(
            writer_, key.c_str(), value.c_str(), handle));
    }
    ReturnCode_t write_string_w_key_w_timestamp(const std::string& key, const std::string& value,
                                                InstanceHandle_t handle, int32_t sec,
                                                uint32_t nanosec) {
        return detail::builtin_return_code(
            zerodds_keyed_string_writer_write_string_w_key_w_timestamp(
                writer_, key.c_str(), value.c_str(), handle, sec, nanosec));
    }
    ReturnCode_t dispose_w_key(const std::string& key) {
        return detail::builtin_return_code(
            zerodds_keyed_string_writer_dispose_w_key(writer_, key.c_str()));
    }
    ReturnCode_t dispose_w_key_w_timestamp(const std::string& key, int32_t sec,
                                           uint32_t nanosec) {
        return detail::builtin_return_code(zerodds_keyed_string_writer_dispose_w_key_w_timestamp(
            writer_, key.c_str(), sec, nanosec));
    }
    ReturnCode_t get_key_value_w_key(std::string& key, InstanceHandle_t handle) {
        char* text = nullptr;
        const ReturnCode_t result = detail::builtin_return_code(
            zerodds_keyed_string_writer_get_key_value_w_key(writer_, handle, &text));
        if (result == RETCODE_OK) {
            key = detail::builtin_take_string(text);
        }
        return result;
    }
    /// The handle of the key's instance, 0 for a key that is not registered.
    InstanceHandle_t lookup_instance_w_key(const std::string& key) {
        InstanceHandle_t handle = 0;
        if (zerodds_keyed_string_writer_lookup_instance_w_key(writer_, key.c_str(), &handle) != 0) {
            return 0;
        }
        return handle;
    }
};

/// `BytesDataWriter` (Annex E).
class BytesDataWriter : public detail::BuiltinWriterBase<2> {
public:
    using BuiltinWriterBase::BuiltinWriterBase;

    /// `write_w_bytes(bytes, offset, length, handle)`: the octets `offset .. offset + length`.
    ReturnCode_t write_w_bytes(const std::vector<uint8_t>& bytes, int32_t offset, int32_t length,
                               InstanceHandle_t handle = 0) {
        return detail::builtin_return_code(zerodds_bytes_writer_write_w_bytes(
            writer_, bytes.data(), bytes.size(), offset, length, handle));
    }
    ReturnCode_t write_w_bytes_w_timestamp(const std::vector<uint8_t>& bytes, int32_t offset,
                                           int32_t length, InstanceHandle_t handle, int32_t sec,
                                           uint32_t nanosec) {
        return detail::builtin_return_code(zerodds_bytes_writer_write_w_bytes_w_timestamp(
            writer_, bytes.data(), bytes.size(), offset, length, handle, sec, nanosec));
    }
};

/// `KeyedBytesDataWriter` (Annex E).
class KeyedBytesDataWriter : public detail::BuiltinWriterBase<3> {
public:
    using BuiltinWriterBase::BuiltinWriterBase;

    ReturnCode_t register_instance_w_key(const std::string& key, InstanceHandle_t& handle) {
        return detail::builtin_return_code(
            zerodds_keyed_bytes_writer_register_instance_w_key(writer_, key.c_str(), &handle));
    }
    ReturnCode_t register_instance_w_key_w_timestamp(const std::string& key, int32_t sec,
                                                     uint32_t nanosec, InstanceHandle_t& handle) {
        return detail::builtin_return_code(
            zerodds_keyed_bytes_writer_register_instance_w_key_w_timestamp(
                writer_, key.c_str(), sec, nanosec, &handle));
    }
    ReturnCode_t unregister_instance_w_key(const std::string& key) {
        return detail::builtin_return_code(
            zerodds_keyed_bytes_writer_unregister_instance_w_key(writer_, key.c_str()));
    }
    ReturnCode_t unregister_instance_w_key_w_timestamp(const std::string& key, int32_t sec,
                                                       uint32_t nanosec) {
        return detail::builtin_return_code(
            zerodds_keyed_bytes_writer_unregister_instance_w_key_w_timestamp(
                writer_, key.c_str(), sec, nanosec));
    }
    /// `write_bytes_w_key(key, bytes, offset, length, handle)`.
    ReturnCode_t write_bytes_w_key(const std::string& key, const std::vector<uint8_t>& bytes,
                                   int32_t offset, int32_t length, InstanceHandle_t handle = 0) {
        return detail::builtin_return_code(zerodds_keyed_bytes_writer_write_bytes_w_key(
            writer_, key.c_str(), bytes.data(), bytes.size(), offset, length, handle));
    }
    ReturnCode_t write_bytes_w_key_w_timestamp(const std::string& key,
                                               const std::vector<uint8_t>& bytes, int32_t offset,
                                               int32_t length, InstanceHandle_t handle, int32_t sec,
                                               uint32_t nanosec) {
        return detail::builtin_return_code(zerodds_keyed_bytes_writer_write_bytes_w_key_w_timestamp(
            writer_, key.c_str(), bytes.data(), bytes.size(), offset, length, handle, sec,
            nanosec));
    }
    ReturnCode_t dispose_w_key(const std::string& key) {
        return detail::builtin_return_code(
            zerodds_keyed_bytes_writer_dispose_w_key(writer_, key.c_str()));
    }
    ReturnCode_t dispose_w_key_w_timestamp(const std::string& key, int32_t sec, uint32_t nanosec) {
        return detail::builtin_return_code(zerodds_keyed_bytes_writer_dispose_w_key_w_timestamp(
            writer_, key.c_str(), sec, nanosec));
    }
    ReturnCode_t get_key_value_w_key(std::string& key, InstanceHandle_t handle) {
        char* text = nullptr;
        const ReturnCode_t result = detail::builtin_return_code(
            zerodds_keyed_bytes_writer_get_key_value_w_key(writer_, handle, &text));
        if (result == RETCODE_OK) {
            key = detail::builtin_take_string(text);
        }
        return result;
    }
    InstanceHandle_t lookup_instance_w_key(const std::string& key) {
        InstanceHandle_t handle = 0;
        if (zerodds_keyed_bytes_writer_lookup_instance_w_key(writer_, key.c_str(), &handle) != 0) {
            return 0;
        }
        return handle;
    }
};

/// `StringDataReader` (Annex E): the next sample, taken or read. RETCODE_NO_DATA when there is
/// none. `info.valid_data` is false for a sample with no data; `value` is empty then.
class StringDataReader : public detail::BuiltinReaderBase<0> {
public:
    using BuiltinReaderBase::BuiltinReaderBase;

    ReturnCode_t take_next(std::string& value, BuiltinSampleInfo& info) {
        return next(true, value, info);
    }
    ReturnCode_t read_next(std::string& value, BuiltinSampleInfo& info) {
        return next(false, value, info);
    }

private:
    ReturnCode_t next(bool destructive, std::string& value, BuiltinSampleInfo& info) {
        char* text = nullptr;
        zerodds_ZeroDdsSampleInfo raw;
        const int status = destructive ? zerodds_string_reader_take_next(reader_, &text, &raw)
                                       : zerodds_string_reader_read_next(reader_, &text, &raw);
        if (status != 0) {
            return detail::builtin_return_code(status);
        }
        info = detail::builtin_sample_info(raw);
        value = detail::builtin_take_string(text);
        return RETCODE_OK;
    }
};

/// `KeyedStringDataReader` (Annex E).
class KeyedStringDataReader : public detail::BuiltinReaderBase<1> {
public:
    using BuiltinReaderBase::BuiltinReaderBase;

    ReturnCode_t take_next(std::string& key, std::string& value, BuiltinSampleInfo& info) {
        return next(true, key, value, info);
    }
    ReturnCode_t read_next(std::string& key, std::string& value, BuiltinSampleInfo& info) {
        return next(false, key, value, info);
    }

private:
    ReturnCode_t next(bool destructive, std::string& key, std::string& value,
                      BuiltinSampleInfo& info) {
        char* key_text = nullptr;
        char* value_text = nullptr;
        zerodds_ZeroDdsSampleInfo raw;
        const int status =
            destructive
                ? zerodds_keyed_string_reader_take_next(reader_, &key_text, &value_text, &raw)
                : zerodds_keyed_string_reader_read_next(reader_, &key_text, &value_text, &raw);
        if (status != 0) {
            return detail::builtin_return_code(status);
        }
        info = detail::builtin_sample_info(raw);
        key = detail::builtin_take_string(key_text);
        value = detail::builtin_take_string(value_text);
        return RETCODE_OK;
    }
};

/// `BytesDataReader` (Annex E).
class BytesDataReader : public detail::BuiltinReaderBase<2> {
public:
    using BuiltinReaderBase::BuiltinReaderBase;

    ReturnCode_t take_next(std::vector<uint8_t>& value, BuiltinSampleInfo& info) {
        return next(true, value, info);
    }
    ReturnCode_t read_next(std::vector<uint8_t>& value, BuiltinSampleInfo& info) {
        return next(false, value, info);
    }

private:
    ReturnCode_t next(bool destructive, std::vector<uint8_t>& value, BuiltinSampleInfo& info) {
        uint8_t* bytes = nullptr;
        uintptr_t length = 0;
        zerodds_ZeroDdsSampleInfo raw;
        const int status = destructive
                               ? zerodds_bytes_reader_take_next(reader_, &bytes, &length, &raw)
                               : zerodds_bytes_reader_read_next(reader_, &bytes, &length, &raw);
        if (status != 0) {
            return detail::builtin_return_code(status);
        }
        info = detail::builtin_sample_info(raw);
        value = detail::builtin_take_bytes(bytes, length);
        return RETCODE_OK;
    }
};

/// `KeyedBytesDataReader` (Annex E).
class KeyedBytesDataReader : public detail::BuiltinReaderBase<3> {
public:
    using BuiltinReaderBase::BuiltinReaderBase;

    ReturnCode_t take_next(std::string& key, std::vector<uint8_t>& value, BuiltinSampleInfo& info) {
        return next(true, key, value, info);
    }
    ReturnCode_t read_next(std::string& key, std::vector<uint8_t>& value, BuiltinSampleInfo& info) {
        return next(false, key, value, info);
    }

private:
    ReturnCode_t next(bool destructive, std::string& key, std::vector<uint8_t>& value,
                      BuiltinSampleInfo& info) {
        char* key_text = nullptr;
        uint8_t* bytes = nullptr;
        uintptr_t length = 0;
        zerodds_ZeroDdsSampleInfo raw;
        const int status =
            destructive
                ? zerodds_keyed_bytes_reader_take_next(reader_, &key_text, &bytes, &length, &raw)
                : zerodds_keyed_bytes_reader_read_next(reader_, &key_text, &bytes, &length, &raw);
        if (status != 0) {
            return detail::builtin_return_code(status);
        }
        info = detail::builtin_sample_info(raw);
        key = detail::builtin_take_string(key_text);
        value = detail::builtin_take_bytes(bytes, length);
        return RETCODE_OK;
    }
};

}  // namespace DDS

#endif  // ZERODDS_BUILTIN_DATA_ENDPOINTS_HPP
