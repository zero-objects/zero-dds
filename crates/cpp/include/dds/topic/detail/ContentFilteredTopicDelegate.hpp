// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/ContentFilteredTopicDelegate.hpp — Delegate for ContentFilteredTopic.

#ifndef ZERODDS_DDS_TOPIC_DETAIL_CONTENTFILTEREDTOPICDELEGATE_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_CONTENTFILTEREDTOPICDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "zerodds/psm/check_status.hpp"
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "dds/topic/detail/TypeObjectsOf.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {
namespace detail {

/// `zerodds_cft_set_type_objects` as the `call` of `call_with_dynamic_contract`.
struct CftSetTypeObjects {
    explicit CftSetTypeObjects(zerodds_ZeroDdsContentFilteredTopic* h) : h_(h) {}
    int operator()(const uint8_t* object, std::size_t len, const uint8_t* const* deps,
                   const std::size_t* lens, std::size_t n, int keyed, uint32_t ext) const {
        return zerodds_cft_set_type_objects(h_, object, len, deps, lens, n, keyed, ext);
    }
    zerodds_ZeroDdsContentFilteredTopic* h_;
};

/// Hands the type of a support that finds it by the topic's type name to the filter.
template <typename S>
inline int cft_set_type(zerodds_ZeroDdsContentFilteredTopic* h, const std::string& type_name,
                        ::zerodds::cxx::integral_constant<int, 0>) {
    return call_with_dynamic_contract(S::contract_for(type_name), CftSetTypeObjects(h));
}

/// A support with dependencies: `zerodds_cft_set_type_objects`.
template <typename S>
inline int cft_set_static_type(zerodds_ZeroDdsContentFilteredTopic* h, const uint8_t* object,
                               std::size_t len, ::zerodds::cxx::integral_constant<bool, true>) {
    const std::size_t n = S::type_object_dependency_count();
    std::vector<const uint8_t*> deps(n);
    std::vector<std::size_t> lens(n);
    for (std::size_t i = 0; i < n; ++i) {
        deps[i] = S::type_object_dependency(i);
        lens[i] = S::type_object_dependency_len(i);
    }
    return zerodds_cft_set_type_objects(h, object, len, n ? vector_data_or_null(deps) : 0,
                                        n ? vector_data_or_null(lens) : 0, n,
                                        S::is_keyed() ? 1 : 0,
                                        static_cast<uint32_t>(S::extensibility()));
}

/// A support without dependencies: `zerodds_cft_set_type_object`.
template <typename S>
inline int cft_set_static_type(zerodds_ZeroDdsContentFilteredTopic* h, const uint8_t* object,
                               std::size_t len, ::zerodds::cxx::integral_constant<bool, false>) {
    return zerodds_cft_set_type_object(h, object, len, S::is_keyed() ? 1 : 0,
                                       static_cast<uint32_t>(S::extensibility()));
}

/// A support with a generated object.
template <typename S>
inline int cft_set_type(zerodds_ZeroDdsContentFilteredTopic* h, const std::string&,
                        ::zerodds::cxx::integral_constant<int, 1>) {
    const uint8_t* object = S::type_object();
    const std::size_t len = S::type_object_len();
    if (object != 0 && len > 0) {
        return cft_set_static_type<S>(
            h, object, len,
            ::zerodds::cxx::integral_constant<bool, has_type_object_dependencies<S>::value>());
    }
    return 0;
}

/// A support without an object: the filter stays untyped.
template <typename S>
inline int cft_set_type(zerodds_ZeroDdsContentFilteredTopic*, const std::string&,
                        ::zerodds::cxx::integral_constant<int, 2>) {
    return 0;
}

/// Delegate for ContentFilteredTopic<T> (DDS-PSM-Cxx 1.0 §7.8).
template <typename T>
class ContentFilteredTopicDelegate {
public:
    ContentFilteredTopicDelegate(const ::dds::domain::DomainParticipant& dp,
                                 const std::string& name,
                                 const ::dds::topic::Topic<T>& related_topic,
                                 const std::string& filter_expression,
                                 const std::vector<std::string>& parameters);
    ~ContentFilteredTopicDelegate();

    // Non-copyable, non-movable (owned by shared_ptr).
    ZERODDS_NONCOPYABLE(ContentFilteredTopicDelegate)
#if ZERODDS_CXX11
    ContentFilteredTopicDelegate(ContentFilteredTopicDelegate&&) = delete;
    ContentFilteredTopicDelegate& operator=(ContentFilteredTopicDelegate&&) = delete;
#endif

    zerodds_ZeroDdsContentFilteredTopic* native_cft_handle() const { return cft_handle_; }
    zerodds_ZeroDdsTopic* native_handle() const { return 0; }
    const std::string& name() const { return name_; }
    const std::string& type_name() const { return related_topic_.delegate()->type_name(); }
    const std::string& filter_expression() const { return filter_expression_; }
    const ::dds::topic::Topic<T>& related_topic() const { return related_topic_; }
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant() const { return participant_; }
    std::vector<std::string> filter_parameters() const;
    template <typename FwdIterator>
    void filter_parameters(const FwdIterator& begin, const FwdIterator& end);

    // Entity operations.
    void enable();
    ::dds::core::status::StatusMask status_changes() const;
    ::dds::core::InstanceHandle instance_handle() const;
    void close();
    void retain();

    /// Registers a new content-filtered topic with its participant (`delete_contained_entities`).
    void enrol(const ::dds::domain::DomainParticipant& dp,
               const ::zerodds::cxx::shared_ptr<ContentFilteredTopicDelegate>& self);

private:
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
    zerodds_ZeroDdsContentFilteredTopic* cft_handle_;
    std::string name_;
    std::string filter_expression_;
    ::dds::topic::Topic<T> related_topic_;
    bool closed_;
};

// Inline implementations (header-only library).

template <typename T>
inline ContentFilteredTopicDelegate<T>::ContentFilteredTopicDelegate(
    const ::dds::domain::DomainParticipant& dp,
    const std::string& name,
    const ::dds::topic::Topic<T>& related_topic,
    const std::string& filter_expression,
    const std::vector<std::string>& parameters)
    : participant_(),
      cft_handle_(0),
      name_(name),
      filter_expression_(filter_expression),
      related_topic_(related_topic),
      closed_(false) {
    participant_ = dp.delegate()->participant();
    std::vector<const char*> raw_params;
    raw_params.reserve(parameters.size());
    for (std::size_t i = 0; i < parameters.size(); ++i) {
        raw_params.push_back(parameters[i].c_str());
    }
    cft_handle_ = zerodds_dp_create_contentfilteredtopic(
        participant_.get(), name.c_str(), related_topic->native_handle(),
        filter_expression.c_str(), vector_data_or_null(raw_params), raw_params.size());
    if (cft_handle_ == 0) {
        throw ::dds::core::Error("ContentFilteredTopicDelegate::create failed");
    }

    // With the type's `CompleteTypeObject` the filter reads every field
    // by name. Without one (a hand-written support, a type XTypes has no
    // object for) a filter that reads a field is refused when the
    // DataReader is created, instead of passing every sample.
    typedef ::dds::topic::topic_type_support<T> support;
    const int rc = cft_set_type<support>(
        cft_handle_, related_topic_->type_name(),
        ::zerodds::cxx::integral_constant<int, type_objects_kind<support>::value>());
    if (rc != 0) {
        zerodds_dp_delete_contentfilteredtopic(participant_.get(), cft_handle_);
        cft_handle_ = 0;
        throw ::dds::core::Error(
            "ContentFilteredTopicDelegate: the type's CompleteTypeObject was refused");
    }
}

template <typename T>
inline ContentFilteredTopicDelegate<T>::~ContentFilteredTopicDelegate() {
    if (!closed_ && cft_handle_ != 0) {
        close();
    }
}

template <typename T>
inline std::vector<std::string> ContentFilteredTopicDelegate<T>::filter_parameters() const {
    std::vector<std::string> out;
    const std::size_t n = zerodds_cft_get_filter_parameter_count(cft_handle_);
    out.reserve(n);
    for (std::size_t i = 0; i < n; ++i) {
        char* p = zerodds_cft_get_filter_parameter(cft_handle_, i);
        if (p == 0) { break; }
        out.push_back(std::string(p));
        zerodds_string_free(p);
    }
    return out;
}

template <typename T>
template <typename FwdIterator>
inline void ContentFilteredTopicDelegate<T>::filter_parameters(const FwdIterator& begin,
                                                                const FwdIterator& end) {
    std::vector<std::string> params(begin, end);
    std::vector<const char*> raw;
    raw.reserve(params.size());
    for (std::size_t i = 0; i < params.size(); ++i) { raw.push_back(params[i].c_str()); }
    if (zerodds_cft_set_filter_parameters(cft_handle_, vector_data_or_null(raw), raw.size()) != 0) {
        throw ::dds::core::Error(
            "ContentFilteredTopicDelegate::filter_parameters: the expression names a missing parameter");
    }
}

template <typename T>
inline void ContentFilteredTopicDelegate<T>::enable() {
    ::zerodds::psm::check_status(zerodds_topic_enable(related_topic_->native_handle()), "ContentFilteredTopicDelegate::enable");
}

template <typename T>
inline ::dds::core::status::StatusMask ContentFilteredTopicDelegate<T>::status_changes() const {
    uint32_t changes = 0;
    ::zerodds::psm::check_status(zerodds_topic_get_status_changes(related_topic_->native_handle(), &changes), "ContentFilteredTopicDelegate::status_changes");
    return ::dds::core::status::StatusMask(changes);
}

template <typename T>
inline ::dds::core::InstanceHandle ContentFilteredTopicDelegate<T>::instance_handle() const {
    uint64_t handle = 0;
    ::zerodds::psm::check_status(zerodds_topic_get_instance_handle(related_topic_->native_handle(), &handle), "ContentFilteredTopicDelegate::instance_handle");
    return ::dds::core::InstanceHandle(handle);
}

template <typename T>
inline void ContentFilteredTopicDelegate<T>::close() {
    if (!closed_ && cft_handle_ != 0 && participant_.get() != 0) {
        zerodds_dp_delete_contentfilteredtopic(participant_.get(), cft_handle_);
        cft_handle_ = 0;
        closed_ = true;
    }
}

template <typename T>
inline void ContentFilteredTopicDelegate<T>::retain() {
    // No-op; shared_ptr lifetime management handles retention.
}

} // namespace detail
} // namespace topic
} // namespace dds

// Include full DomainParticipant definition for method implementations.
#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_TOPIC_DETAIL_CONTENTFILTEREDTOPICDELEGATE_HPP