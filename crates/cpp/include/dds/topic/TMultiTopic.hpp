// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TMultiTopic.hpp — DDS-PSM-Cxx 1.0 §7.8 `TMultiTopic<T, DELEGATE>`.

#ifndef ZERODDS_DDS_TOPIC_TMULTITOPIC_HPP
#define ZERODDS_DDS_TOPIC_TMULTITOPIC_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/core/types.hpp"
#include "dds/topic/TopicDescription.hpp"
#include "dds/topic/detail/MultiTopicDelegate.hpp"
#include "zerodds.h"

namespace dds {
namespace domain {
class DomainParticipant;
}

namespace topic {

namespace detail {

/// The MultiTopic API over any delegate (Spec §7.8): a topic description of the result type `T`
/// joined from the topics its subscription expression names.
template <typename T, typename DELEGATE>
class TMultiTopic : public TopicDescription<T, DELEGATE> {
public:
    typedef DELEGATE DELEGATE_T;
    typedef TopicDescription<T, DELEGATE_T> BASE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// A null MultiTopic.
protected:
    TMultiTopic() ZERODDS_NOEXCEPT {}
public:

    /// A MultiTopic sharing `ref`.
    explicit TMultiTopic(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// The MultiTopic `name` of the participant `dp`: `expression` is the subscription
    /// expression (DDS 1.4 Annex B.2), `[params_begin, params_end)` its `%n` parameters.
    /// `InvalidArgumentError`/`Error` as for the C API's `BAD_PARAMETER`: the expression, the
    /// topics of its FROM clause and `T` do not fit together.
    template <typename FWDIterator>
    TMultiTopic(const ::dds::domain::DomainParticipant& dp, const std::string& name,
                const std::string expression, const FWDIterator& params_begin,
                const FWDIterator& params_end)
        : BASE_T(::zerodds::cxx::make_shared<DELEGATE_T>(dp, name, expression,
                                              std::vector<std::string>(params_begin, params_end))) {
        this->impl_->enrol(dp, this->impl_);
    }

#if ZERODDS_CXX11
    TMultiTopic(const TMultiTopic&) = default;
    TMultiTopic& operator=(const TMultiTopic&) = default;
    TMultiTopic(TMultiTopic&&) = default;
    TMultiTopic& operator=(TMultiTopic&&) = default;
    ~TMultiTopic() = default;
#endif

    /// The subscription expression.
    const std::string expression() const {
        return this->impl_ ? this->delegate()->expression() : std::string();
    }

    /// Replaces the expression parameters.
    template <typename FWDIterator>
    void expression_parameters(const FWDIterator& params_begin, const FWDIterator& params_end) {
        this->delegate()->expression_parameters(params_begin, params_end);
    }

    /// The expression parameters.
    ::dds::core::StringSeq expression_parameters() const {
        const std::vector<std::string> params = this->delegate()->expression_parameters();
        return ::dds::core::StringSeq(params.begin(), params.end());
    }
};

} // namespace detail

} // namespace topic
} // namespace dds

// The header of the OMG that declares `TMultiTopic` declares the class `MultiTopic` in the same
// file: including it is enough to name `dds::topic::MultiTopic`.
#include "dds/topic/MultiTopic.hpp"

#endif // ZERODDS_DDS_TOPIC_TMULTITOPIC_HPP
