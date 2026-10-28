// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/detail/DataReaderDelegate.hpp — Delegate for DataReader.

#ifndef ZERODDS_DDS_SUB_DETAIL_DATAREADERDELEGATE_HPP
#define ZERODDS_DDS_SUB_DETAIL_DATAREADERDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include "dds/core/detail/borrow.hpp"
#include <map>
#include <memory>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/qos_bridge.hpp"
#include "dds/sub/Subscriber.hpp"
#include "dds/sub/LoanedSamples.hpp"
#include "dds/sub/Query.hpp"
#include "dds/sub/Sample.hpp"
#include "dds/sub/detail/DataReaderStatus.hpp"
#include "dds/sub/detail/QosFetch.hpp"
#include "dds/sub/detail/SampleDetail.hpp"
#include "dds/sub/status/DataState.hpp"
#include "dds/topic/AnyTopicDescription.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "dds/topic/detail/ContentFilteredTopicDelegate.hpp"
#include "dds/topic/detail/TypeObjectsOf.hpp"
#include "dds/sub/detail/ReadSpec.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {

template <typename T>
class DataReaderListener;

namespace detail {


/// Whether `X` is a MultiTopic (it has `native_multitopic_handle()`), as against a
/// ContentFilteredTopic (`native_cft_handle()`).
template <typename X>
struct is_multitopic {
    template <typename U>
    static const U& make();
    template <typename U>
    static probe_yes test(probe_size<sizeof((void)make<U>()->native_multitopic_handle(), 0)>*);
    template <typename U>
    static probe_no test(...);
    static const bool value = sizeof(test<X>(0)) == sizeof(probe_yes);
};
template <typename X>
const bool is_multitopic<X>::value;

/// Whether the type support `TS` has a `key_hash(const T&)` of its own.
template <typename TS, typename T>
struct has_key_hash {
    static const T& sample_arg();
    template <typename U>
    static probe_yes test(probe_size<sizeof((void)U::key_hash(sample_arg()), 0)>*);
    template <typename U>
    static probe_no test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(probe_yes);
};
template <typename TS, typename T>
const bool has_key_hash<TS, T>::value;

/// Whether the type support `TS` has a `wire_xcdr2()`.
template <typename TS>
struct has_wire_xcdr2 {
    template <typename U>
    static probe_yes test(probe_size<sizeof((void)U::wire_xcdr2(), 0)>*);
    template <typename U>
    static probe_no test(...);
    static const bool value = sizeof(test<TS>(0)) == sizeof(probe_yes);
};
template <typename TS>
const bool has_wire_xcdr2<TS>::value;

/// The representation `TS::encode` writes: XCDR2 when the type support says so
/// (`wire_xcdr2()`), else XCDR1, little-endian.
template <typename TS>
inline bool wire_xcdr2_of(::zerodds::cxx::integral_constant<bool, true>) {
    return TS::wire_xcdr2() ? true : false;
}
template <typename TS>
inline bool wire_xcdr2_of(::zerodds::cxx::integral_constant<bool, false>) {
    return false;
}

/// The address of the first element of `v`, null when empty.
template <typename V>
inline const typename V::value_type* reader_data_or_null(const V& v) {
    return v.empty() ? 0 : &v[0];
}

/// The bytes of a key hash a type support returns (`std::array`, `std::vector`, ...).
template <typename K>
inline std::vector<uint8_t> reader_key_bytes(const K& k) {
    return std::vector<uint8_t>(k.data(), k.data() + k.size());
}

/// A type support with a `key_hash` of its own.
template <typename TS, typename T>
inline std::vector<uint8_t> reader_key_hash(zerodds_ZeroDdsDataReader*, const T& sample,
                                            ::zerodds::cxx::integral_constant<bool, true>) {
    return reader_key_bytes(TS::key_hash(sample));
}
/// A type support without `key_hash`: the C API reads the key from the encoded sample,
/// against the type objects the reader was given.
template <typename TS, typename T>
inline std::vector<uint8_t> reader_key_hash(zerodds_ZeroDdsDataReader* handle, const T& sample,
                                            ::zerodds::cxx::integral_constant<bool, false>) {
    const std::vector<uint8_t> body = TS::encode(sample);
    std::vector<uint8_t> key(16, 0);
    const int rc = zerodds_dr_sample_key_hash(
        handle, reader_data_or_null(body), body.size(),
        wire_xcdr2_of<TS>(::zerodds::cxx::integral_constant<bool, has_wire_xcdr2<TS>::value>()) ? 1
                                                                                                : 0,
        0, &key[0]);
    ::zerodds::psm::check_status(rc, "DataReader::lookup_instance");
    return key;
}

/// The sample of the support `topic_type_support<T>` decodes from `buf`: a support that finds
/// the type by the topic's type name decodes with it ...
template <typename T>
inline T decode_reader_sample(const std::string& type_name, const uint8_t* buf, std::size_t len,
                              ::zerodds::wire::Encoding version, bool big_endian,
                              ::zerodds::cxx::integral_constant<bool, true>) {
    return ::dds::topic::topic_type_support<T>::decode_for(type_name, buf, len, version,
                                                           big_endian);
}
/// ... and any other one without.
template <typename T>
inline T decode_reader_sample(const std::string&, const uint8_t* buf, std::size_t len,
                              ::zerodds::wire::Encoding version, bool big_endian,
                              ::zerodds::cxx::integral_constant<bool, false>) {
    return ::dds::sub::detail::decode_sample<T>(buf, len, version, big_endian);
}

/// Hands the CompleteTypeObject of the reader's type to the reader.
class ReaderTypeObjectsSetter {
public:
    explicit ReaderTypeObjectsSetter(zerodds_ZeroDdsDataReader* handle) : handle_(handle) {}
    int operator()(const uint8_t* object, std::size_t len, const uint8_t* const* deps,
                   const std::size_t* lens, std::size_t n, int keyed, uint32_t extensibility) const {
        return zerodds_dr_set_type_objects(handle_, object, len, deps, lens, n, keyed,
                                           extensibility);
    }

private:
    zerodds_ZeroDdsDataReader* handle_;
};

/// Creates the reader with the type object of its type (`*out` stays null when the call is
/// not made).
class TypedReaderCreator {
public:
    TypedReaderCreator(zerodds_ZeroDdsSubscriber* sub, zerodds_ZeroDdsTopic* topic,
                       const zerodds_ZeroDdsDataReaderQos* qos, zerodds_ZeroDdsDataReader** out)
        : sub_(sub), topic_(topic), qos_(qos), out_(out) {}
    int operator()(const uint8_t* object, std::size_t len, const uint8_t* const*,
                   const std::size_t*, std::size_t, int keyed, uint32_t) const {
        *out_ = zerodds_sub_create_datareader_typed_wire(sub_, topic_, qos_, object, len, keyed, 2);
        return 0;
    }

private:
    zerodds_ZeroDdsSubscriber* sub_;
    zerodds_ZeroDdsTopic* topic_;
    const zerodds_ZeroDdsDataReaderQos* qos_;
    zerodds_ZeroDdsDataReader** out_;
};

/// Releases the samples of a read on a MultiTopic.
class MultiTopicSamplesGuard {
public:
    MultiTopicSamplesGuard(zerodds_ZeroDdsMultiTopicSample* samples, uintptr_t count)
        : samples_(samples), count_(count) {}
    ~MultiTopicSamplesGuard() { zerodds_multitopic_samples_free(samples_, count_); }

private:
    MultiTopicSamplesGuard(const MultiTopicSamplesGuard&);
    MultiTopicSamplesGuard& operator=(const MultiTopicSamplesGuard&);
    zerodds_ZeroDdsMultiTopicSample* samples_;
    uintptr_t count_;
};

/// Delegate for DataReader<T> (DDS-PSM-Cxx 1.0 §7.10).
template <typename T>
class DataReaderDelegate {
public:
    DataReaderDelegate(const ::dds::sub::Subscriber& sub, const ::dds::topic::Topic<T>& topic);
    DataReaderDelegate(const ::dds::sub::Subscriber& sub, const ::dds::topic::Topic<T>& topic,
                       const ::dds::sub::qos::DataReaderQos& qos);
    // CFT-aware constructors.
    template <typename CftT>
    DataReaderDelegate(const ::dds::sub::Subscriber& sub, const CftT& cft);
    template <typename CftT>
    DataReaderDelegate(const ::dds::sub::Subscriber& sub, const CftT& cft,
                       const ::dds::sub::qos::DataReaderQos& qos);
    /// Borrow constructor for listener bridge — does not own the handle.
    explicit DataReaderDelegate(zerodds_ZeroDdsDataReader* h);
    ~DataReaderDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(DataReaderDelegate)
#if ZERODDS_CXX11
    DataReaderDelegate(DataReaderDelegate&&) = delete;
    DataReaderDelegate& operator=(DataReaderDelegate&&) = delete;
#endif

    zerodds_ZeroDdsDataReader* native_handle() const { return handle_; }
    const ::dds::sub::Subscriber& subscriber() const { return sub_; }
    ::dds::topic::AnyTopicDescription topic_description() const { return description_; }

    // Entity operations.
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    // DataReader operations.
    ::dds::sub::LoanedSamples<T> take(const ReadSpec& spec) {
        return ::dds::sub::LoanedSamples<T>(fetch(spec, true));
    }
    ::dds::sub::LoanedSamples<T> read(const ReadSpec& spec) {
        return ::dds::sub::LoanedSamples<T>(fetch(spec, false));
    }
    /// The samples a read or take returns, for the iterator forms.
    std::vector< ::dds::sub::Sample<T> > fetch(const ReadSpec& spec, bool take);
    const ::dds::sub::status::DataState& default_filter_state() const { return default_state_; }
    void default_filter_state(const ::dds::sub::status::DataState& s) { default_state_ = s; }
    ::dds::core::InstanceHandle lookup_instance(const T& key) const;
    T& key_value(T& sample, const ::dds::core::InstanceHandle& h);
    void wait_for_historical_data(const ::dds::core::Duration& timeout);
    void wait_for_matched(int32_t min, const ::dds::core::Duration& d);

    const ::dds::sub::qos::DataReaderQos& qos() const;
    void qos(const ::dds::sub::qos::DataReaderQos& q);

    // Status getters; each keeps its result until the next call.
    const ::dds::core::status::SubscriptionMatchedStatus& subscription_matched_status() {
        subscription_matched_ = subscription_matched_status_of(handle_);
        return subscription_matched_;
    }
    const ::dds::core::status::SampleLostStatus& sample_lost_status() {
        sample_lost_ = sample_lost_status_of(handle_);
        return sample_lost_;
    }
    const ::dds::core::status::RequestedDeadlineMissedStatus& requested_deadline_missed_status() {
        requested_deadline_missed_ = requested_deadline_missed_status_of(handle_);
        return requested_deadline_missed_;
    }
    const ::dds::core::status::LivelinessChangedStatus& liveliness_changed_status() {
        liveliness_changed_ = liveliness_changed_status_of(handle_);
        return liveliness_changed_;
    }
    const ::dds::core::status::SampleRejectedStatus& sample_rejected_status() {
        sample_rejected_ = sample_rejected_status_of(handle_);
        return sample_rejected_;
    }
    const ::dds::core::status::RequestedIncompatibleQosStatus& requested_incompatible_qos_status() {
        requested_incompatible_qos_ = requested_incompatible_qos_status_of(handle_);
        return requested_incompatible_qos_;
    }

    // Listener support.
    /// Installs `l` for `mask`, `nullptr` removes it. Defined in DataReader.hpp.
    void listener(::dds::sub::DataReaderListener<T>* l, const ::dds::core::status::StatusMask& mask);
    void set_listener(::dds::sub::DataReaderListener<T>* l) { listener_ = l; }
    ::dds::sub::DataReaderListener<T>* listener() const { return listener_; }
    void attach_listener(const zerodds_ZeroDdsDataReaderListener& vt, uint32_t mask);
    void detach_listener();

private:
    static zerodds_ZeroDdsDataReader* create_reader(zerodds_ZeroDdsSubscriber* sub,
                                                     zerodds_ZeroDdsTopic* topic,
                                                     const std::string& type_name,
                                                     const zerodds_ZeroDdsDataReaderQos* qos);

    /// The reader on a ContentFilteredTopic or a MultiTopic, by what `cft` is.
    template <typename CftT>
    void create_from(const ::dds::sub::Subscriber& sub, const CftT& cft,
                     const zerodds_ZeroDdsDataReaderQos* qos,
                     ::zerodds::cxx::integral_constant<bool, true>) {
        joined_ = zerodds_sub_create_multitopic_reader(sub->native_handle(),
                                                       cft->native_multitopic_handle(), qos);
        if (joined_ == 0) {
            throw ::dds::core::Error(qos == 0 ? "DataReaderDelegate::create (MultiTopic) failed"
                                              : "DataReaderDelegate::create (MultiTopic) with QoS failed");
        }
    }
    template <typename CftT>
    void create_from(const ::dds::sub::Subscriber& sub, const CftT& cft,
                     const zerodds_ZeroDdsDataReaderQos* qos,
                     ::zerodds::cxx::integral_constant<bool, false>) {
        handle_ = zerodds_sub_create_datareader_with_cft(sub->native_handle(),
                                                         cft->native_cft_handle(), qos);
        if (handle_ == 0) {
            throw ::dds::core::Error(qos == 0 ? "DataReaderDelegate::create (CFT) failed"
                                              : "DataReaderDelegate::create (CFT) with QoS failed");
        }
    }

    void tell_the_type(const std::string& type_name);
    void init(const ::dds::sub::Subscriber& sub, zerodds_ZeroDdsTopic* topic,
              const std::string& type_name,
              const zerodds_ZeroDdsDataReaderQos* qos);
    void remember(const ::dds::core::InstanceHandle& h, const T& sample) {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(samples_mutex_);
        samples_.insert(std::make_pair(h.value(), sample));
    }

    ::dds::sub::Subscriber sub_;
    ::dds::topic::AnyTopicDescription description_;
    /// Decodes the body of a MultiTopic sample, XCDR2 little-endian.
    T decode_joined(const uint8_t* buf, size_t len) const;
    /// `read` and `take` of a reader on a MultiTopic.
    std::vector< ::dds::sub::Sample<T> > fetch_joined(const ReadSpec& spec, bool take);

    zerodds_ZeroDdsDataReader* handle_;
    /// The reader of a DataReader created on a MultiTopic (no `handle_` then).
    zerodds_ZeroDdsMultiTopicReader* joined_;
    bool owns_;
    zerodds_ZeroDdsDataReaderListener* listener_reg_;
    ::dds::sub::DataReaderListener<T>* listener_;
    bool closed_;
    ::dds::sub::status::DataState default_state_;
    mutable ::dds::sub::qos::DataReaderQos qos_;
    // The first sample seen of each instance, for key_value().
    ::zerodds::cxx::mutex samples_mutex_;
    std::map<uint64_t, T> samples_;
    ::dds::core::status::SubscriptionMatchedStatus subscription_matched_;
    ::dds::core::status::SampleLostStatus sample_lost_;
    ::dds::core::status::RequestedDeadlineMissedStatus requested_deadline_missed_;
    ::dds::core::status::LivelinessChangedStatus liveliness_changed_;
    ::dds::core::status::SampleRejectedStatus sample_rejected_;
    ::dds::core::status::RequestedIncompatibleQosStatus requested_incompatible_qos_;
};

// Inline implementations (header-only library).

template <typename T>
inline void DataReaderDelegate<T>::init(const ::dds::sub::Subscriber& sub,
                                        zerodds_ZeroDdsTopic* topic, const std::string& type_name,
                                        const zerodds_ZeroDdsDataReaderQos* qos) {
    sub_ = sub;
    handle_ = create_reader(sub->native_handle(), topic, type_name, qos);
    if (handle_ == 0) {
        throw ::dds::core::Error("DataReaderDelegate::create failed");
    }
    tell_the_type(type_name);
}

// A QueryCondition reads the fields its expression names from the type, so the reader is told
// the type's CompleteTypeObject. A hand-written support has none; a query that names a field
// is then refused when it is evaluated.
template <typename T>
inline void DataReaderDelegate<T>::tell_the_type(const std::string& type_name) {
    // -1000: the support has no object (a hand-written one); the reader stays untyped.
    const int rc = ::dds::topic::detail::with_type_objects<T>(type_name,
                                                               ReaderTypeObjectsSetter(handle_));
    if (rc != -1000) {
        ::zerodds::psm::check_status(rc, "DataReaderDelegate: the type's CompleteTypeObject");
    }
}

template <typename T>
inline DataReaderDelegate<T>::DataReaderDelegate(const ::dds::sub::Subscriber& sub,
                                                 const ::dds::topic::Topic<T>& topic)
    : sub_(::dds::core::null),
      description_(topic),
      handle_(0),
      joined_(0),
      owns_(true),
      listener_reg_(0),
      listener_(0),
      closed_(false) {
    init(sub, topic->native_handle(), topic->type_name(), 0);
}

template <typename T>
inline DataReaderDelegate<T>::DataReaderDelegate(const ::dds::sub::Subscriber& sub,
                                                 const ::dds::topic::Topic<T>& topic,
                                                 const ::dds::sub::qos::DataReaderQos& qos)
    : sub_(::dds::core::null),
      description_(topic),
      handle_(0),
      joined_(0),
      owns_(true),
      listener_reg_(0),
      listener_(0),
      closed_(false) {
    const zerodds_ZeroDdsDataReaderQos native = ::dds::core::detail::to_native(qos);
    init(sub, topic->native_handle(), topic->type_name(), &native);
}

// CFT-aware constructors.
template <typename T>
template <typename CftT>
inline DataReaderDelegate<T>::DataReaderDelegate(const ::dds::sub::Subscriber& sub,
                                                 const CftT& cft)
    : sub_(::dds::core::null),
      description_(cft),
      handle_(0),
      joined_(0),
      owns_(true),
      listener_reg_(0),
      listener_(0),
      closed_(false) {
    sub_ = sub;
    create_from(sub, cft, 0,
                ::zerodds::cxx::integral_constant<bool, is_multitopic<CftT>::value>());
}

template <typename T>
template <typename CftT>
inline DataReaderDelegate<T>::DataReaderDelegate(const ::dds::sub::Subscriber& sub,
                                                 const CftT& cft,
                                                 const ::dds::sub::qos::DataReaderQos& qos)
    : sub_(::dds::core::null),
      description_(cft),
      handle_(0),
      joined_(0),
      owns_(true),
      listener_reg_(0),
      listener_(0),
      closed_(false) {
    sub_ = sub;
    const zerodds_ZeroDdsDataReaderQos native = ::dds::core::detail::to_native(qos);
    create_from(sub, cft, &native,
                ::zerodds::cxx::integral_constant<bool, is_multitopic<CftT>::value>());
}

template <typename T>
inline DataReaderDelegate<T>::DataReaderDelegate(zerodds_ZeroDdsDataReader* h)
    : sub_(::dds::core::detail::borrow< ::dds::sub::Subscriber>(zerodds_dr_get_subscriber(h))),
      description_(::dds::core::detail::borrow< ::dds::topic::Topic<T> >(
          zerodds_dr_get_topicdescription(h))),
      handle_(h),
      joined_(0),
      owns_(false),
      listener_reg_(0),
      listener_(0),
      closed_(false) {}

template <typename T>
inline DataReaderDelegate<T>::~DataReaderDelegate() {
    if (joined_ != 0) {
        zerodds_multitopic_reader_destroy(joined_);
        joined_ = 0;
    }
    if (owns_ && !closed_ && handle_ != 0) {
        detach_listener();
        zerodds_sub_delete_datareader(sub_->native_handle(), handle_);
        handle_ = 0;
    }
}

template <typename T>
inline zerodds_ZeroDdsDataReader* DataReaderDelegate<T>::create_reader(
    zerodds_ZeroDdsSubscriber* sub, zerodds_ZeroDdsTopic* topic, const std::string& type_name,
    const zerodds_ZeroDdsDataReaderQos* qos) {
    // The generated or dynamic type object makes a typed reader; -1000: the support has none (a
    // hand-written one) or an empty one, and the reader is untyped.
    zerodds_ZeroDdsDataReader* typed = 0;
    if (::dds::topic::detail::with_type_objects<T>(type_name,
                                                   TypedReaderCreator(sub, topic, qos, &typed)) ==
        -1000) {
        return zerodds_sub_create_datareader(sub, topic, qos);
    }
    return typed;
}

template <typename T>
inline void DataReaderDelegate<T>::enable() {
    ::zerodds::psm::check_status(zerodds_dr_enable(handle_), "DataReaderDelegate::enable");
}

template <typename T>
inline ::dds::core::status::StatusMask DataReaderDelegate<T>::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_dr_get_status_changes(handle_, &changes),
                              "DataReaderDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

template <typename T>
inline ::dds::core::InstanceHandle DataReaderDelegate<T>::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_dr_get_instance_handle(handle_, &handle),
                              "DataReaderDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

template <typename T>
inline void DataReaderDelegate<T>::close() {
    if (joined_ != 0) {
        zerodds_multitopic_reader_destroy(joined_);
        joined_ = 0;
        closed_ = true;
    }
    if (owns_ && !closed_ && handle_ != 0) {
        detach_listener();
        zerodds_sub_delete_datareader(sub_->native_handle(), handle_);
        handle_ = 0;
        closed_ = true;
    }
}

template <typename T>
inline void DataReaderDelegate<T>::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

template <typename T>
inline std::vector< ::dds::sub::Sample<T> > DataReaderDelegate<T>::fetch(const ReadSpec& spec,
                                                                      bool take) {
    if (joined_ != 0) {
        return fetch_joined(spec, take);
    }
    const uint32_t ss = static_cast<uint32_t>(spec.state.sample_state().to_ulong());
    const uint32_t vs = static_cast<uint32_t>(spec.state.view_state().to_ulong());
    const uint32_t is = static_cast<uint32_t>(spec.state.instance_state().to_ulong());
    const size_t max = spec.max_samples;
    const char* what = take ? "DataReaderDelegate::take" : "DataReaderDelegate::read";

    zerodds_ZeroDdsSampleArray arr = zerodds_ZeroDdsSampleArray();
    int rc = 0;
    if (spec.query != 0) {
        std::vector<const char*> raw_params;
        raw_params.reserve(spec.query->parameters_length());
        for (::dds::sub::Query::const_iterator p = spec.query->begin(); p != spec.query->end(); ++p) {
            raw_params.push_back(p->c_str());
        }
        void* cond = zerodds_dr_create_querycondition(
            handle_, ss, vs, is, spec.query->expression().c_str(),
            raw_params.empty() ? 0 : &raw_params[0], raw_params.size());
        if (cond == 0) {
            throw ::dds::core::InvalidArgumentError("DataReaderDelegate: the query is not valid");
        }
        if (spec.scope == ReadSpec::Scope::Instance) {
            rc = take ? zerodds_dr_take_instance_w_condition(handle_, spec.instance.value(), &arr,
                                                             max, cond)
                      : zerodds_dr_read_instance_w_condition(handle_, spec.instance.value(), &arr,
                                                             max, cond);
        } else if (spec.scope == ReadSpec::Scope::NextInstance) {
            rc = take ? zerodds_dr_take_next_instance_w_condition(handle_, spec.instance.value(),
                                                                  &arr, max, cond)
                      : zerodds_dr_read_next_instance_w_condition(handle_, spec.instance.value(),
                                                                  &arr, max, cond);
        } else {
            rc = take ? zerodds_dr_take_w_condition(handle_, &arr, max, cond)
                      : zerodds_dr_read_w_condition(handle_, &arr, max, cond);
        }
        zerodds_dr_delete_readcondition(handle_, cond);
    } else if (spec.scope == ReadSpec::Scope::Instance) {
        rc = take ? zerodds_dr_take_instance(handle_, spec.instance.value(), &arr, max, ss, vs, is)
                  : zerodds_dr_read_instance(handle_, spec.instance.value(), &arr, max, ss, vs, is);
    } else if (spec.scope == ReadSpec::Scope::NextInstance) {
        rc = take ? zerodds_dr_take_next_instance(handle_, spec.instance.value(), &arr, max, ss,
                                                  vs, is)
                  : zerodds_dr_read_next_instance(handle_, spec.instance.value(), &arr, max, ss,
                                                  vs, is);
    } else {
        rc = take ? zerodds_dr_take(handle_, &arr, max, ss, vs, is)
                  : zerodds_dr_read(handle_, &arr, max, ss, vs, is);
    }
    if (rc == -7 /*NoData*/) {
        return std::vector< ::dds::sub::Sample<T> >();
    }
    ::zerodds::psm::check_status(rc, what);

    std::vector< ::dds::sub::Sample<T> > out;
    out.reserve(arr.count);
    {
        ::dds::sub::detail::LoanGuard guard(handle_, &arr);
        for (size_t i = 0; i < arr.count; ++i) {
            const uint8_t* buf = arr.buffers[i];
            size_t len = arr.lengths[i];
            T data = T();
            if (arr.infos[i].valid_data) {
                const ::zerodds::wire::Encoding version = arr.infos[i].representation == 1
                    ? ::zerodds::wire::Encoding::PlainCdr2
                    : ::zerodds::wire::Encoding::PlainCdr1;
                typedef ::dds::topic::topic_type_support<T> TS;
                data = decode_reader_sample<T>(
                    description_.type_name(), buf, len, version, arr.infos[i].big_endian != 0,
                    ::zerodds::cxx::integral_constant<
                        bool, ::dds::topic::detail::has_dynamic_contract<TS>::value>());
            }
            if (arr.infos[i].valid_data) {
                remember(::dds::core::InstanceHandle(arr.infos[i].instance_handle), data);
            }
            out.push_back(::dds::sub::Sample<T>(
                ZERODDS_MOVE(data), ::dds::sub::detail::make_sample_info(arr.infos[i])));
        }
        guard.release();
    }
    zerodds_dr_return_loan(handle_, &arr);
    return out;
}

template <typename T>
inline T DataReaderDelegate<T>::decode_joined(const uint8_t* buf, size_t len) const {
    typedef ::dds::topic::topic_type_support<T> TS;
    const ::zerodds::wire::Encoding version = ::zerodds::wire::Encoding::PlainCdr2;
    return decode_reader_sample<T>(
        description_.type_name(), buf, len, version, false,
        ::zerodds::cxx::integral_constant<bool,
                                          ::dds::topic::detail::has_dynamic_contract<TS>::value>());
}

template <typename T>
inline std::vector< ::dds::sub::Sample<T> > DataReaderDelegate<T>::fetch_joined(const ReadSpec& spec,
                                                                           bool take) {
    const uint32_t ss = static_cast<uint32_t>(spec.state.sample_state().to_ulong());
    const uint32_t vs = static_cast<uint32_t>(spec.state.view_state().to_ulong());
    const uint32_t is = static_cast<uint32_t>(spec.state.instance_state().to_ulong());
    std::vector<const char*> raw_params;
    const char* expression = 0;
    if (spec.query != 0) {
        expression = spec.query->expression().c_str();
        raw_params.reserve(spec.query->parameters_length());
        for (::dds::sub::Query::const_iterator p = spec.query->begin(); p != spec.query->end(); ++p) {
            raw_params.push_back(p->c_str());
        }
    }
    const int scope = spec.scope == ReadSpec::Scope::All ? 0
                      : spec.scope == ReadSpec::Scope::Instance ? 1 : 2;
    zerodds_ZeroDdsMultiTopicSample* raw = 0;
    uintptr_t count = 0;
    const int rc = zerodds_multitopic_reader_read_ex(
        joined_, take ? 1 : 0, scope, spec.instance.value(), ss, vs, is,
        spec.max_samples == 0 ? -1 : static_cast<int32_t>(spec.max_samples), expression,
        raw_params.empty() ? 0 : &raw_params[0], raw_params.size(), &raw, &count);
    if (rc == -13) { // ZERODDS_UNSUPPORTED: a query with a read by instance
        throw ::dds::core::UnsupportedError(
            "DataReader on a MultiTopic: a query is provided for a read of all instances only");
    }
    ::zerodds::psm::check_status(rc, take ? "DataReaderDelegate::take" : "DataReaderDelegate::read");
    MultiTopicSamplesGuard release(raw, count);
    std::vector< ::dds::sub::Sample<T> > out;
    out.reserve(count);
    for (uintptr_t i = 0; i < count; ++i) {
        T data = T();
        if (raw[i].info.valid_data) {
            data = decode_joined(raw[i].data, raw[i].len);
        }
        out.push_back(::dds::sub::Sample<T>(
            ZERODDS_MOVE(data), ::dds::sub::detail::make_sample_info(raw[i].info)));
    }
    return out;
}

template <typename T>
inline void DataReaderDelegate<T>::wait_for_historical_data(const ::dds::core::Duration& timeout) {
    int rc = zerodds_dr_wait_for_historical_data(
        handle_, ::dds::core::detail::is_infinite(timeout) ? 0x7fffffff : timeout.sec(),
        ::dds::core::detail::is_infinite(timeout) ? 999999999u : timeout.nanosec());
    if (rc == -4) throw ::dds::core::TimeoutError("DataReaderDelegate::wait_for_historical_data");
    ::zerodds::psm::check_status(rc, "DataReaderDelegate::wait_for_historical_data");
}

template <typename T>
inline void DataReaderDelegate<T>::wait_for_matched(int32_t min, const ::dds::core::Duration& d) {
    uint64_t ms = ::dds::core::detail::is_infinite(d) ? ZERODDS_BLOCKING_INFINITE
                                  : static_cast<uint64_t>(d.to_millisecs());
    int rc = zerodds_dr_wait_for_matched(handle_, min, ms);
    if (rc == -4) throw ::dds::core::TimeoutError("DataReaderDelegate::wait_for_matched");
    ::zerodds::psm::check_status(rc, "DataReaderDelegate::wait_for_matched");
}

template <typename T>
inline ::dds::core::InstanceHandle DataReaderDelegate<T>::lookup_instance(const T& key) const {
    typedef ::dds::topic::topic_type_support<T> TS;
    const std::vector<uint8_t> kh = reader_key_hash<TS>(
        handle_, key,
        ::zerodds::cxx::integral_constant<bool, has_key_hash<TS, T>::value>());
    uint64_t h = 0;
    ::zerodds::psm::check_status(zerodds_dr_lookup_instance(handle_, reader_data_or_null(kh),
                                                            kh.size(), &h),
                              "DataReaderDelegate::lookup_instance");
    return ::dds::core::InstanceHandle(h);
}

template <typename T>
inline T& DataReaderDelegate<T>::key_value(T& sample, const ::dds::core::InstanceHandle& h) {
    // The C API holds the key as encoded key members, which the type support
    // cannot decode into a T; the instances this reader returned a sample of
    // are remembered with the first one.
    {
        ::zerodds::cxx::lock_guard< ::zerodds::cxx::mutex> lock(samples_mutex_);
        typename std::map<uint64_t, T>::const_iterator it = samples_.find(h.value());
        if (it != samples_.end()) {
            sample = it->second;
            return sample;
        }
    }
    uint8_t probe[1];
    size_t len = sizeof(probe);
    int rc = zerodds_dr_get_key_value(handle_, h.value(), probe, &len);
    // A buffer too small is a handle that names an instance.
    if (rc != 0 && rc != -8) {
        ::zerodds::psm::check_status(rc, "DataReaderDelegate::key_value");
    }
    throw ::dds::core::UnsupportedError(
        "DataReader::key_value: no sample of the instance was returned by this DataReader object");
}

template <typename T>
inline const ::dds::sub::qos::DataReaderQos& DataReaderDelegate<T>::qos() const {
    qos_ = fetch_datareader_qos(handle_, "DataReaderDelegate::qos");
    return qos_;
}

template <typename T>
inline void DataReaderDelegate<T>::qos(const ::dds::sub::qos::DataReaderQos& q) {
    zerodds_ZeroDdsDataReaderQos native = ::dds::core::detail::to_native(q);
    ::zerodds::psm::check_status(zerodds_dr_set_qos(handle_, &native), "DataReaderDelegate::qos");
    qos_ = q;
}

template <typename T>
inline void DataReaderDelegate<T>::attach_listener(
    const zerodds_ZeroDdsDataReaderListener& vt, uint32_t mask) {
    detach_listener();
    listener_reg_ = new zerodds_ZeroDdsDataReaderListener(vt);
    ::zerodds::psm::check_status(zerodds_dr_set_listener(handle_, listener_reg_, mask),
                              "DataReaderDelegate::attach_listener");
}

template <typename T>
inline void DataReaderDelegate<T>::detach_listener() {
    if (listener_reg_ != 0) {
        zerodds_dr_set_listener(handle_, 0, 0);
        delete listener_reg_;
        listener_reg_ = 0;
    }
}

} // namespace detail
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_DETAIL_DATAREADERDELEGATE_HPP