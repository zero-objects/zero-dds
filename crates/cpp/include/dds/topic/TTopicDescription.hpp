// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TTopicDescription.hpp — DDS-PSM-Cxx 1.0 §7.8 `TTopicDescription<T, DELEGATE>`.

#ifndef ZERODDS_DDS_TOPIC_TTOPICDESCRIPTION_HPP
#define ZERODDS_DDS_TOPIC_TTOPICDESCRIPTION_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>

#include "dds/core/TEntity.hpp"
#include "dds/topic/TopicTraits.hpp"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {

namespace detail {

/// The topic description API over any delegate (Spec §7.8).
/// Reference type with delegate pattern (DDS-PSM-Cxx 1.0 §7.5.1.1, §7.5.1.2).
template <typename T, typename DELEGATE>
class TTopicDescription : public ::dds::core::TEntity<DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef ::dds::core::TEntity<DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    typedef T DataType;

    /// Constructs a null topic description.
protected:
    TTopicDescription() ZERODDS_NOEXCEPT {}
public:

    /// Construct from delegate shared_ptr.
    explicit TTopicDescription(const DELEGATE_REF_T& ref) ZERODDS_NOEXCEPT : BASE_T(ref) {}

    /// Reference semantics: copy/assign share the delegate.
#if ZERODDS_CXX11
    TTopicDescription(const TTopicDescription&) = default;
    TTopicDescription& operator=(const TTopicDescription&) = default;
    TTopicDescription(TTopicDescription&&) = default;
    TTopicDescription& operator=(TTopicDescription&&) = default;
    ~TTopicDescription() = default;
#endif

    /// Topic name (Spec §7.8).
    std::string name() const {
        if (this->impl_) {
            return this->delegate()->name();
        }
        return std::string();
    }

    /// Type name (Spec §7.8).
    std::string type_name() const {
        if (this->impl_) {
            return this->delegate()->type_name();
        }
        return std::string();
    }

    /// Domain participant (Spec §7.8).
    ::dds::domain::DomainParticipant domain_participant() const;

};

} // namespace detail

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TTOPICDESCRIPTION_HPP