// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/MultiTopic.hpp — DDS-PSM-Cxx 1.0 §7.8.

#ifndef ZERODDS_DDS_TOPIC_MULTITOPIC_HPP
#define ZERODDS_DDS_TOPIC_MULTITOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/detail/conformance.hpp"
#include "dds/topic/TMultiTopic.hpp"
#include "dds/topic/detail/MultiTopicDelegate.hpp"
#include "dds/domain/DomainParticipant.hpp"

namespace dds {
namespace topic {

/// MultiTopic<T> (Spec §7.8): reference type with delegate pattern (DDS-PSM-Cxx 1.0 §7.5.1.1).
template <typename T, template <typename Q> class DELEGATE = dds::topic::detail::MultiTopicDelegate>
class MultiTopic : public detail::TMultiTopic<T, DELEGATE<T> > {
public:
    typedef DELEGATE<T> DELEGATE_T;
    typedef detail::TMultiTopic<T, DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A MultiTopic sharing `ref`.
    explicit MultiTopic(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// The MultiTopic `name` of the participant `dp` (see `TMultiTopic`).
    template <typename FWDIterator>
    MultiTopic(const ::dds::domain::DomainParticipant& dp, const std::string& name,
               const std::string expression, const FWDIterator& params_begin,
               const FWDIterator& params_end)
        : BASE_T(dp, name, expression, params_begin, params_end) {}

    /// A null MultiTopic.
protected:
    MultiTopic() ZERODDS_NOEXCEPT {}
public:

    /// The null MultiTopic (`dds::core::null`, DDS-PSM-Cxx 1.0 §7.5.1.2).
    MultiTopic(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    MultiTopic(const MultiTopic&) = default;
    MultiTopic& operator=(const MultiTopic&) = default;
    MultiTopic(MultiTopic&&) = default;
    MultiTopic& operator=(MultiTopic&&) = default;
    ~MultiTopic() = default;
#endif
};

namespace detail {
template <typename T>
inline void MultiTopicDelegate<T>::enrol(const ::dds::domain::DomainParticipant& dp,
                                         const ::zerodds::cxx::shared_ptr<MultiTopicDelegate>& self) {
    dp->child_topics().add(self, DelegateCloser<MultiTopicDelegate>());
}
} // namespace detail

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_MULTITOPIC_HPP
