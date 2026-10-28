// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/cond/TReadCondition.hpp — DDS-PSM-Cxx 1.0 `TReadCondition<D>`.

#ifndef ZERODDS_DDS_SUB_COND_TREADCONDITION_HPP
#define ZERODDS_DDS_SUB_COND_TREADCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include "dds/core/cond/TCondition.hpp"
#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/status/DataState.hpp"

namespace dds {
namespace sub {
namespace cond {

/// What ReadCondition and QueryCondition share: the state they wait for and
/// the reader.
template <typename DELEGATE>
class TReadCondition : public ::dds::core::cond::TCondition<DELEGATE> {
public:
    typedef ::dds::core::cond::TCondition<DELEGATE> BASE_T;
    typedef DELEGATE DELEGATE_T;
    typedef typename BASE_T::DELEGATE_REF_T DELEGATE_REF_T;

    /// The sample, view and instance states the condition waits for.
    const ::dds::sub::status::DataState state_filter() const {
        return this->delegate()->state_filter();
    }

    /// The reader the condition is bound to.
    const ::dds::sub::AnyDataReader& data_reader() const { return this->delegate()->data_reader(); }

protected:
    TReadCondition() {}
    explicit TReadCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}
};

} // namespace cond
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_COND_TREADCONDITION_HPP
