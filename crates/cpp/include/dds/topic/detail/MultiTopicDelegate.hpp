// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/detail/MultiTopicDelegate.hpp — Delegate for MultiTopic (DDS-PSM-Cxx 1.0 §7.8).

#ifndef ZERODDS_DDS_TOPIC_DETAIL_MULTITOPICDELEGATE_HPP
#define ZERODDS_DDS_TOPIC_DETAIL_MULTITOPICDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/InstanceHandle.hpp"
#include "dds/core/status/Status.hpp"
#include "dds/topic/TopicTraits.hpp"
#include "dds/topic/Topic.hpp"
#include "dds/topic/detail/TypeObjectsOf.hpp"
#include "zerodds.h"
#include "zerodds/psm/check_status.hpp"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {
namespace detail {

/// `zerodds_dp_create_multitopic` as the `call` of `with_type_objects`; the handle it creates
/// and the status it reports go to `created` and `status`.
struct CreateMultiTopic {
    CreateMultiTopic(zerodds_ZeroDdsDomainParticipant* participant, const std::string& name,
                     const std::string& type_name, const std::string& expression,
                     const std::vector<const char*>& raw, zerodds_ZeroDdsMultiTopic*& created,
                     int& status)
        : participant_(participant),
          name_(name),
          type_name_(type_name),
          expression_(expression),
          raw_(raw),
          created_(created),
          status_(status) {}
    int operator()(const uint8_t* object, std::size_t len, const uint8_t* const* deps,
                   const std::size_t* lens, std::size_t n, int keyed, uint32_t ext) const {
        created_ = zerodds_dp_create_multitopic(
            participant_, name_.c_str(), type_name_.c_str(), expression_.c_str(),
            vector_data_or_null(raw_), raw_.size(), object, len, deps, lens, n, keyed, ext,
            &status_);
        return created_ == 0 ? 1 : 0;
    }
    zerodds_ZeroDdsDomainParticipant* participant_;
    const std::string& name_;
    const std::string& type_name_;
    const std::string& expression_;
    const std::vector<const char*>& raw_;
    zerodds_ZeroDdsMultiTopic*& created_;
    int& status_;
};

/// Delegate for MultiTopic<T>: the MultiTopic `name` of the result type `T`, selected by the
/// subscription expression of DDS 1.4 Annex B.2 over the topics of its `FROM` clause.
template <typename T>
class MultiTopicDelegate {
public:
    MultiTopicDelegate(const ::dds::domain::DomainParticipant& dp, const std::string& name,
                       const std::string& expression, const std::vector<std::string>& parameters);
    ~MultiTopicDelegate();

    ZERODDS_NONCOPYABLE(MultiTopicDelegate)
#if ZERODDS_CXX11
    MultiTopicDelegate(MultiTopicDelegate&&) = delete;
    MultiTopicDelegate& operator=(MultiTopicDelegate&&) = delete;
#endif

    zerodds_ZeroDdsMultiTopic* native_multitopic_handle() const { return handle_; }
    zerodds_ZeroDdsTopic* native_handle() const { return 0; }
    const std::string& name() const { return name_; }
    const std::string& type_name() const { return type_name_; }
    const std::string& expression() const { return expression_; }
    std::vector<std::string> expression_parameters() const { return parameters_; }
    template <typename FwdIterator>
    void expression_parameters(const FwdIterator& begin, const FwdIterator& end);
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant() const { return participant_; }

    // Entity operations.
    void enable() {}
    ::dds::core::status::StatusMask status_changes() const { return ::dds::core::status::StatusMask(); }
    ::dds::core::InstanceHandle instance_handle() const { return ::dds::core::InstanceHandle(); }
    /// `delete_multitopic`; `PreconditionNotMetError` while a reader created on it exists.
    void close();
    void retain() {}

    /// Registers a new MultiTopic with its participant (`delete_contained_entities`).
    void enrol(const ::dds::domain::DomainParticipant& dp,
               const ::zerodds::cxx::shared_ptr<MultiTopicDelegate>& self);

private:
    ::zerodds::cxx::shared_ptr<zerodds_ZeroDdsDomainParticipant> participant_;
    zerodds_ZeroDdsMultiTopic* handle_;
    std::string name_;
    std::string type_name_;
    std::string expression_;
    std::vector<std::string> parameters_;
    bool closed_;
};

template <typename T>
inline MultiTopicDelegate<T>::MultiTopicDelegate(const ::dds::domain::DomainParticipant& dp,
                                                 const std::string& name,
                                                 const std::string& expression,
                                                 const std::vector<std::string>& parameters)
    : participant_(),
      handle_(0),
      name_(name),
      type_name_(),
      expression_(expression),
      parameters_(parameters),
      closed_(false) {
    participant_ = dp.delegate()->participant();
    type_name_ = get_type_name<T>();
    std::vector<const char*> raw;
    raw.reserve(parameters.size());
    for (std::size_t i = 0; i < parameters.size(); ++i) {
        raw.push_back(parameters[i].c_str());
    }
    int status = 0;
    zerodds_ZeroDdsDomainParticipant* participant = participant_.get();
    zerodds_ZeroDdsMultiTopic* created = 0;
    const int rc = with_type_objects<T>(
        type_name_,
        CreateMultiTopic(participant, name, type_name_, expression, raw, created, status));
    if (rc == -1000) {
        throw ::dds::core::InvalidArgumentError(
            "MultiTopic: the result type has no CompleteTypeObject (a hand-written type support)");
    }
    if (created == 0) {
        // BAD_PARAMETER: the expression, the topics of its FROM clause and the result type do not
        // fit together (DDS 1.4 §2.2.2.2.1.9).
        ::zerodds::psm::check_status(status != 0 ? status : -1, "MultiTopicDelegate::create");
        throw ::dds::core::Error("MultiTopicDelegate::create failed");
    }
    handle_ = created;
}

template <typename T>
inline MultiTopicDelegate<T>::~MultiTopicDelegate() {
    if (!closed_ && handle_ != 0) {
        // A reader created on it keeps it: the handle then lives until the process ends.
        if (zerodds_dp_delete_multitopic(participant_.get(), handle_) == 0) {
            handle_ = 0;
        }
        closed_ = true;
    }
}

template <typename T>
template <typename FwdIterator>
inline void MultiTopicDelegate<T>::expression_parameters(const FwdIterator& begin,
                                                         const FwdIterator& end) {
    std::vector<std::string> params(begin, end);
    std::vector<const char*> raw;
    for (std::size_t i = 0; i < params.size(); ++i) {
        raw.push_back(params[i].c_str());
    }
    ::zerodds::psm::check_status(
        zerodds_multitopic_set_parameters(handle_, vector_data_or_null(raw), raw.size()),
        "MultiTopicDelegate::expression_parameters");
    parameters_ = ZERODDS_MOVE(params);
}

template <typename T>
inline void MultiTopicDelegate<T>::close() {
    if (!closed_ && handle_ != 0) {
        ::zerodds::psm::check_status(zerodds_dp_delete_multitopic(participant_.get(), handle_),
                                     "MultiTopicDelegate::close");
        handle_ = 0;
        closed_ = true;
    }
}

} // namespace detail
} // namespace topic
} // namespace dds

#include "dds/domain/DomainParticipant.hpp"

#endif // ZERODDS_DDS_TOPIC_DETAIL_MULTITOPICDELEGATE_HPP
