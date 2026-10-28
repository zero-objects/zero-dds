// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/DataWriterDelegate.hpp — Delegate for DataWriter.

#ifndef ZERODDS_DDS_PUB_DETAIL_DATAWRITERDELEGATE_HPP
#define ZERODDS_DDS_PUB_DETAIL_DATAWRITERDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "dds/core/detail/borrow.hpp"
#include <map>
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/pub/Publisher.hpp"
#include "dds/pub/detail/DataWriterStatus.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/TopicInstance.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

template <typename T>
class DataWriterListener;

namespace detail {

/// The unused-type tags of the detection trait below (ISO C++03 has no `decltype`/`void_t`).
typedef char KeyHashYes;
struct KeyHashNo {
    char pad[2];
};
template <size_t N>
struct KeyHashSize {};

/// Whether the type support `TS` has a `key_hash(const T&)` of its own.
template <typename TS, typename T>
struct has_key_hash {
    static const T& sample_arg();
    template <typename U>
    static KeyHashYes test(KeyHashSize<sizeof((void)U::key_hash(sample_arg()), 0)>*);
    template <typename U>
    static KeyHashNo test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(KeyHashYes);
};
template <typename TS, typename T>
const bool has_key_hash<TS, T>::value;

/// The bytes of a key hash a type support returns (`std::array`, `std::vector`, ...).
template <typename K>
inline std::vector<uint8_t> key_hash_bytes(const K& k) {
    return std::vector<uint8_t>(k.data(), k.data() + k.size());
}

/// The address of the first byte of `v`, null when empty.
template <typename V>
inline const uint8_t* bytes_of(const V& v) {
    return v.empty() ? 0 : &v[0];
}

/// Delegate for DataWriter<T> (DDS-PSM-Cxx 1.0 §7.9).
template <typename T>
class DataWriterDelegate {
public:
    DataWriterDelegate(const ::dds::pub::Publisher& pub, const ::dds::topic::Topic<T>& topic);
    DataWriterDelegate(const ::dds::pub::Publisher& pub, const ::dds::topic::Topic<T>& topic,
                       const ::dds::pub::qos::DataWriterQos& qos);
    /// Borrow constructor for listener bridge — does not own the handle.
    explicit DataWriterDelegate(zerodds_ZeroDdsDataWriter* h);
    ~DataWriterDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(DataWriterDelegate)
#if ZERODDS_CXX11
    DataWriterDelegate(DataWriterDelegate&&) = delete;
    DataWriterDelegate& operator=(DataWriterDelegate&&) = delete;
#endif

    zerodds_ZeroDdsDataWriter* native_handle() const { return handle_; }
    const ::dds::pub::Publisher& publisher() const { return pub_; }
    const ::dds::topic::Topic<T>& topic() const { return topic_; }

    // Entity operations.
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    // DataWriter operations.
    void write(const T& sample, const ::dds::core::InstanceHandle& handle,
               const ::dds::core::Time* ts);
    ::dds::core::InstanceHandle register_instance(const T& sample);
    ::dds::core::InstanceHandle register_instance(const T& sample, const ::dds::core::Time& ts);
    ::dds::core::InstanceHandle lookup_instance(const T& sample);
    void dispose_instance(const ::dds::core::InstanceHandle& h, const ::dds::core::Time* ts);
    void unregister_instance(const ::dds::core::InstanceHandle& h, const ::dds::core::Time* ts);
    T& key_value(T& sample, const ::dds::core::InstanceHandle& h);
    void wait_for_acknowledgments(const ::dds::core::Duration& d);
    void assert_liveliness();
    void wait_for_matched(int32_t min, const ::dds::core::Duration& d);

    const ::dds::pub::qos::DataWriterQos& qos() const;
    void qos(const ::dds::pub::qos::DataWriterQos& q);

    // Status getters.
    ::dds::core::status::PublicationMatchedStatus publication_matched_status() {
        return publication_matched_status_of(handle_);
    }
    ::dds::core::status::LivelinessLostStatus liveliness_lost_status() {
        return liveliness_lost_status_of(handle_);
    }
    ::dds::core::status::OfferedDeadlineMissedStatus offered_deadline_missed_status() {
        return offered_deadline_missed_status_of(handle_);
    }
    ::dds::core::status::OfferedIncompatibleQosStatus offered_incompatible_qos_status() {
        return offered_incompatible_qos_status_of(handle_);
    }

    // Listener support.
    void attach_listener(const zerodds_ZeroDdsDataWriterListener& vt, uint32_t mask);
    void detach_listener();
    void set_listener(::dds::pub::DataWriterListener<T>* l) { listener_ = l; }
    ::dds::pub::DataWriterListener<T>* listener() const { return listener_; }

private:
    static zerodds_ZeroDdsDataWriter* create_writer(zerodds_ZeroDdsPublisher* pub,
                                                     zerodds_ZeroDdsTopic* topic,
                                                     const std::string& type_name,
                                                     const zerodds_ZeroDdsDataWriterQos* qos);

    /// A type support with a `key_hash` of its own.
    /// The type is a DynamicType known by the topic's type name.
    template <typename TS>
    static zerodds_ZeroDdsDataWriter* create_writer_for(
        zerodds_ZeroDdsPublisher* pub, zerodds_ZeroDdsTopic* topic, const std::string& type_name,
        const zerodds_ZeroDdsDataWriterQos* qos, ::zerodds::cxx::integral_constant<int, 2>) {
        return create_dynamic_writer(pub, topic, qos, TS::contract_for(type_name));
    }

    template <typename Contract>
    static zerodds_ZeroDdsDataWriter* create_dynamic_writer(zerodds_ZeroDdsPublisher* pub,
                                                            zerodds_ZeroDdsTopic* topic,
                                                            const zerodds_ZeroDdsDataWriterQos* qos,
                                                            const Contract& contract) {
        return create_dynamic_writer_object(pub, topic, qos, contract,
                                            contract->handle->type_object());
    }

    template <typename Contract, typename Object>
    static zerodds_ZeroDdsDataWriter* create_dynamic_writer_object(
        zerodds_ZeroDdsPublisher* pub, zerodds_ZeroDdsTopic* topic,
        const zerodds_ZeroDdsDataWriterQos* qos, const Contract& contract, const Object& object) {
        return zerodds_pub_create_datawriter_typed_wire(
            pub, topic, qos, bytes_of(object), object.size(), contract->handle->keyed() ? 1 : 0,
            contract->handle->extensibility(), 2);
    }

    /// The type carries a wire contract.
    template <typename TS>
    static zerodds_ZeroDdsDataWriter* create_writer_for(
        zerodds_ZeroDdsPublisher* pub, zerodds_ZeroDdsTopic* topic, const std::string&,
        const zerodds_ZeroDdsDataWriterQos* qos, ::zerodds::cxx::integral_constant<int, 1>) {
        if (TS::type_object() != 0 && TS::type_object_len() > 0) {
            return zerodds_pub_create_datawriter_typed_wire(
                pub, topic, qos, TS::type_object(), TS::type_object_len(),
                TS::is_keyed() ? 1 : 0, ::dds::topic::detail::wire_extensibility<TS>(), 2);
        }
        return zerodds_pub_create_datawriter(pub, topic, qos);
    }

    /// An untyped writer.
    template <typename TS>
    static zerodds_ZeroDdsDataWriter* create_writer_for(
        zerodds_ZeroDdsPublisher* pub, zerodds_ZeroDdsTopic* topic, const std::string&,
        const zerodds_ZeroDdsDataWriterQos* qos, ::zerodds::cxx::integral_constant<int, 0>) {
        return zerodds_pub_create_datawriter(pub, topic, qos);
    }

    template <typename TS>
    std::vector<uint8_t> own_key(const T& sample, ::zerodds::cxx::integral_constant<bool, true>) const {
        return key_hash_bytes(TS::key_hash(sample));
    }

    /// A type support without: the C API reads the key from the encoded sample.
    template <typename TS>
    std::vector<uint8_t> own_key(const T& sample, ::zerodds::cxx::integral_constant<bool, false>) const {
        const std::vector<uint8_t> body = TS::encode(sample);
        std::vector<uint8_t> key(16, 0);
        int rc = zerodds_dw_sample_key_hash(handle_, bytes_of(body), body.size(), &key[0]);
        ::zerodds::psm::check_status(rc, "DataWriterDelegate::key_hash");
        return key;
    }

    std::vector<uint8_t> key_of(const T& sample) const {
        typedef ::dds::topic::topic_type_support<T> TS;
        return own_key<TS>(sample,
                           ::zerodds::cxx::integral_constant<bool, has_key_hash<TS, T>::value>());
    }

    /// The key hash of the instance `h` names.
    std::vector<uint8_t> key_of_handle(const ::dds::core::InstanceHandle& h) const {
        std::vector<uint8_t> key(16, 0);
        ::zerodds::psm::check_status(zerodds_dw_instance_key_hash(handle_, h.value(), &key[0]),
                                  "DataWriterDelegate: the handle names no instance of this writer");
        return key;
    }

    void remember(const ::dds::core::InstanceHandle& h, const T& sample) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(samples_mutex_);
        samples_[h.value()] = sample;
    }

    void init(const ::dds::pub::Publisher& pub, const ::dds::topic::Topic<T>& topic,
              const zerodds_ZeroDdsDataWriterQos* qos);

    ::dds::pub::Publisher pub_;
    ::dds::topic::Topic<T> topic_;
    zerodds_ZeroDdsDataWriter* handle_;
    bool owns_;
    zerodds_ZeroDdsDataWriterListener* listener_reg_;
    ::dds::pub::DataWriterListener<T>* listener_;
    mutable ::dds::pub::qos::DataWriterQos qos_;
    // The sample each instance was registered or looked up with, for
    // key_value().
    ::zerodds::cxx::mutex samples_mutex_;
    std::map<uint64_t, T> samples_;
    bool closed_;
};

// Inline implementations (header-only library).

template <typename T>
inline void DataWriterDelegate<T>::init(const ::dds::pub::Publisher& pub,
                                        const ::dds::topic::Topic<T>& topic,
                                        const zerodds_ZeroDdsDataWriterQos* qos) {
    pub_ = pub;
    topic_ = topic;
    handle_ = create_writer(pub->native_handle(), topic->native_handle(), topic->type_name(), qos);
    if (handle_ == 0) {
        throw ::dds::core::Error("DataWriterDelegate::create failed");
    }
}

template <typename T>
inline DataWriterDelegate<T>::DataWriterDelegate(const ::dds::pub::Publisher& pub,
                                                 const ::dds::topic::Topic<T>& topic)
    : pub_(::dds::core::null),
      topic_(::dds::core::null),
      handle_(0),
      owns_(true),
      listener_reg_(0),
      listener_(0),
      qos_(),
      samples_mutex_(),
      samples_(),
      closed_(false) {
    init(pub, topic, 0);
}

template <typename T>
inline DataWriterDelegate<T>::DataWriterDelegate(const ::dds::pub::Publisher& pub,
                                                 const ::dds::topic::Topic<T>& topic,
                                                 const ::dds::pub::qos::DataWriterQos& qos)
    : pub_(::dds::core::null),
      topic_(::dds::core::null),
      handle_(0),
      owns_(true),
      listener_reg_(0),
      listener_(0),
      qos_(),
      samples_mutex_(),
      samples_(),
      closed_(false) {
    zerodds_ZeroDdsDataWriterQos native = ::dds::core::detail::to_native(qos);
    init(pub, topic, &native);
}

template <typename T>
inline DataWriterDelegate<T>::DataWriterDelegate(zerodds_ZeroDdsDataWriter* h)
    : pub_(::dds::core::detail::borrow< ::dds::pub::Publisher>(zerodds_dw_get_publisher(h))),
      topic_(::dds::core::detail::borrow< ::dds::topic::Topic<T> >(zerodds_dw_get_topic(h))),
      handle_(h),
      owns_(false),
      listener_reg_(0),
      listener_(0),
      qos_(),
      samples_mutex_(),
      samples_(),
      closed_(false) {}

template <typename T>
inline DataWriterDelegate<T>::~DataWriterDelegate() {
    if (owns_ && !closed_ && handle_ != 0) {
        detach_listener();
        zerodds_pub_delete_datawriter(pub_->native_handle(), handle_);
        handle_ = 0;
    }
}

template <typename T>
inline zerodds_ZeroDdsDataWriter* DataWriterDelegate<T>::create_writer(
    zerodds_ZeroDdsPublisher* pub, zerodds_ZeroDdsTopic* topic, const std::string& type_name,
    const zerodds_ZeroDdsDataWriterQos* qos) {
    typedef ::dds::topic::topic_type_support<T> TS;
    // 2: the type is a DynamicType known by the topic's type name; 1: the type carries a wire
    // contract; 0: an untyped writer.
    const int mode = ::dds::topic::detail::has_dynamic_contract<TS>::value
                         ? 2
                         : (::dds::topic::detail::has_wire_contract<TS>::value ? 1 : 0);
    return create_writer_for<TS>(pub, topic, type_name, qos,
                                 ::zerodds::cxx::integral_constant<int, mode>());
}

template <typename T>
inline void DataWriterDelegate<T>::enable() {
    ::zerodds::psm::check_status(zerodds_dw_enable(handle_), "DataWriterDelegate::enable");
}

template <typename T>
inline ::dds::core::status::StatusMask DataWriterDelegate<T>::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_dw_get_status_changes(handle_, &changes),
                              "DataWriterDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

template <typename T>
inline ::dds::core::InstanceHandle DataWriterDelegate<T>::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_dw_get_instance_handle(handle_, &handle),
                              "DataWriterDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

template <typename T>
inline void DataWriterDelegate<T>::close() {
    if (owns_ && !closed_ && handle_ != 0) {
        detach_listener();
        zerodds_pub_delete_datawriter(pub_->native_handle(), handle_);
        handle_ = 0;
        closed_ = true;
    }
}

template <typename T>
inline void DataWriterDelegate<T>::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

template <typename T>
inline void DataWriterDelegate<T>::write(const T& sample, const ::dds::core::InstanceHandle& handle,
                                         const ::dds::core::Time* ts) {
    // DDS 1.4 §2.2.2.4.2.11: a handle that is not the sample's instance is
    // an error, not a write to another instance.
    if (!handle.is_nil()) {
        const ::dds::core::InstanceHandle own = lookup_instance(sample);
        if (own != handle) {
            throw ::dds::core::InvalidArgumentError(
                "DataWriter::write: the handle is not the instance of the sample");
        }
    }
    const std::vector<uint8_t> buf = ::dds::topic::topic_type_support<T>::encode(sample);
    int rc = ts == 0
                 ? zerodds_dw_write(handle_, bytes_of(buf), buf.size(), handle.value())
                 : zerodds_dw_write_w_timestamp(handle_, bytes_of(buf), buf.size(), handle.value(),
                                                ::dds::core::detail::seconds_of(*ts), ts->nanosec());
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::write");
}

template <typename T>
inline ::dds::core::InstanceHandle DataWriterDelegate<T>::register_instance(const T& sample) {
    const std::vector<uint8_t> kh = key_of(sample);
    uint64_t h = 0;
    int rc = zerodds_dw_register_instance(handle_, bytes_of(kh), kh.size(), &h);
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::register_instance");
    remember(::dds::core::InstanceHandle(h), sample);
    return ::dds::core::InstanceHandle(h);
}

template <typename T>
inline ::dds::core::InstanceHandle DataWriterDelegate<T>::register_instance(
    const T& sample, const ::dds::core::Time& ts) {
    const std::vector<uint8_t> kh = key_of(sample);
    uint64_t h = 0;
    int rc = zerodds_dw_register_instance_w_timestamp(
        handle_, bytes_of(kh), kh.size(), ::dds::core::detail::seconds_of(ts), ts.nanosec(), &h);
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::register_instance@ts");
    remember(::dds::core::InstanceHandle(h), sample);
    return ::dds::core::InstanceHandle(h);
}

template <typename T>
inline ::dds::core::InstanceHandle DataWriterDelegate<T>::lookup_instance(const T& sample) {
    const std::vector<uint8_t> kh = key_of(sample);
    uint64_t h = 0;
    int rc = zerodds_dw_lookup_instance(handle_, bytes_of(kh), kh.size(), &h);
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::lookup_instance");
    if (h != 0) {
        remember(::dds::core::InstanceHandle(h), sample);
    }
    return ::dds::core::InstanceHandle(h);
}

template <typename T>
inline void DataWriterDelegate<T>::dispose_instance(const ::dds::core::InstanceHandle& h,
                                                    const ::dds::core::Time* ts) {
    std::vector<uint8_t> key = key_of_handle(h);
    int rc = ts == 0 ? zerodds_dw_dispose(handle_, &key[0], h.value())
                           : zerodds_dw_dispose_w_timestamp(handle_, &key[0], h.value(),
                                                            ::dds::core::detail::seconds_of(*ts), ts->nanosec());
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::dispose_instance");
}

template <typename T>
inline void DataWriterDelegate<T>::unregister_instance(const ::dds::core::InstanceHandle& h,
                                                       const ::dds::core::Time* ts) {
    int rc = ts == 0
                 ? zerodds_dw_unregister_instance(handle_, h.value())
                 : zerodds_dw_unregister_instance_w_timestamp(handle_, h.value(), ::dds::core::detail::seconds_of(*ts),
                                                              ts->nanosec());
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::unregister_instance");
}

template <typename T>
inline T& DataWriterDelegate<T>::key_value(T& sample, const ::dds::core::InstanceHandle& h) {
    // The C API holds the key as encoded key members, which the type support
    // cannot decode into a T; the instances this writer registered or looked
    // up are remembered with their sample.
    {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(samples_mutex_);
        typename std::map<uint64_t, T>::const_iterator it = samples_.find(h.value());
        if (it != samples_.end()) {
            sample = it->second;
            return sample;
        }
    }
    std::vector<uint8_t> ignored(16, 0);
    ::zerodds::psm::check_status(zerodds_dw_instance_key_hash(handle_, h.value(), &ignored[0]),
                              "DataWriterDelegate::key_value");
    throw ::dds::core::UnsupportedError(
        "DataWriter::key_value: the instance was not registered through this DataWriter object");
}

template <typename T>
inline void DataWriterDelegate<T>::wait_for_acknowledgments(const ::dds::core::Duration& d) {
    uint64_t ms = ::dds::core::detail::is_infinite(d) ? ZERODDS_BLOCKING_INFINITE
                                  : static_cast<uint64_t>(d.to_millisecs());
    int rc = zerodds_dw_wait_for_acknowledgments(handle_, ms);
    if (rc == -4) throw ::dds::core::TimeoutError("DataWriterDelegate::wait_for_acks");
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::wait_for_acknowledgments");
}

template <typename T>
inline void DataWriterDelegate<T>::assert_liveliness() {
    ::zerodds::psm::check_status(zerodds_dw_assert_liveliness(handle_),
                              "DataWriterDelegate::assert_liveliness");
}

template <typename T>
inline void DataWriterDelegate<T>::wait_for_matched(int32_t min,
                                                    const ::dds::core::Duration& d) {
    uint64_t ms = ::dds::core::detail::is_infinite(d) ? ZERODDS_BLOCKING_INFINITE
                                  : static_cast<uint64_t>(d.to_millisecs());
    int rc = zerodds_dw_wait_for_matched(handle_, min, ms);
    if (rc == -4) throw ::dds::core::TimeoutError("DataWriterDelegate::wait_for_matched");
    ::zerodds::psm::check_status(rc, "DataWriterDelegate::wait_for_matched");
}

template <typename T>
inline const ::dds::pub::qos::DataWriterQos& DataWriterDelegate<T>::qos() const {
    qos_ = ::dds::core::detail::fetch_qos(
        ::dds::core::detail::make_qos_getter(zerodds_dw_get_qos, handle_),
        &::dds::core::detail::datawriter_qos_of, "DataWriterDelegate::qos",
        ::dds::core::detail::OctetMembers<zerodds_ZeroDdsDataWriterQos>()(&zerodds_ZeroDdsDataWriterQos::user_data)(&zerodds_ZeroDdsDataWriterQos::topic_data)(&zerodds_ZeroDdsDataWriterQos::group_data));
    return qos_;
}

template <typename T>
inline void DataWriterDelegate<T>::qos(const ::dds::pub::qos::DataWriterQos& q) {
    zerodds_ZeroDdsDataWriterQos native = ::dds::core::detail::to_native(q);
    ::zerodds::psm::check_status(zerodds_dw_set_qos(handle_, &native), "DataWriterDelegate::qos");
    qos_ = q;
}

template <typename T>
inline void DataWriterDelegate<T>::attach_listener(
    const zerodds_ZeroDdsDataWriterListener& vt, uint32_t mask) {
    detach_listener();
    listener_reg_ = new zerodds_ZeroDdsDataWriterListener(vt);
    ::zerodds::psm::check_status(zerodds_dw_set_listener(handle_, listener_reg_, mask),
                              "DataWriterDelegate::attach_listener");
}

template <typename T>
inline void DataWriterDelegate<T>::detach_listener() {
    if (listener_reg_ != 0) {
        zerodds_dw_set_listener(handle_, 0, 0);
        delete listener_reg_;
        listener_reg_ = 0;
    }
}

} // namespace detail
} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_DATAWRITERDELEGATE_HPP
