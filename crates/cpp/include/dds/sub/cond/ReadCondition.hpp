// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/cond/ReadCondition.hpp + QueryCondition — DDS-PSM-Cxx 1.0 §7.5.10.6/7.

#ifndef ZERODDS_DDS_SUB_COND_READCONDITION_HPP
#define ZERODDS_DDS_SUB_COND_READCONDITION_HPP

#include <string>
#include <vector>

#include "dds/core/cond/Condition.hpp"
#include "dds/sub/Subscriber.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace cond {

/// `ReadCondition<T>` (Spec §7.5.10.6).
template <typename T>
class ReadCondition : public ::dds::core::cond::Condition {
public:
    /// Konstruiert ueber DataReader + state-mask.
    ReadCondition(DataReader<T>& dr, uint32_t sample_states,
                  uint32_t view_states, uint32_t instance_states)
        : Condition(zerodds_dr_create_readcondition(dr.native_handle(),
                                                    sample_states, view_states,
                                                    instance_states)) {
        if (handle_ == nullptr) {
            throw ::dds::core::Error("ReadCondition::create failed");
        }
        reader_ = dr.native_handle();
    }
    ~ReadCondition() override {
        if (handle_ != nullptr && reader_ != nullptr) {
            zerodds_dr_delete_readcondition(reader_, handle_);
            handle_ = nullptr;
        }
    }

private:
    zerodds_ZeroDdsDataReader* reader_{nullptr};
};

/// `QueryCondition<T>` (Spec §7.5.10.7).
template <typename T>
class QueryCondition : public ::dds::core::cond::Condition {
public:
    /// Konstruiert mit Filter-Expression + Parameter.
    QueryCondition(DataReader<T>& dr, uint32_t sample_states,
                   uint32_t view_states, uint32_t instance_states,
                   const std::string& expression,
                   const std::vector<std::string>& parameters)
        : Condition(nullptr) {
        std::vector<const char*> raw_params;
        raw_params.reserve(parameters.size());
        for (const auto& p : parameters) raw_params.push_back(p.c_str());
        handle_ = zerodds_dr_create_querycondition(
            dr.native_handle(), sample_states, view_states, instance_states,
            expression.c_str(),
            raw_params.empty() ? nullptr : raw_params.data(),
            raw_params.size());
        if (handle_ == nullptr) {
            throw ::dds::core::Error("QueryCondition::create failed");
        }
        reader_ = dr.native_handle();
    }
    ~QueryCondition() override {
        if (handle_ != nullptr && reader_ != nullptr) {
            zerodds_dr_delete_readcondition(reader_, handle_);
            handle_ = nullptr;
        }
    }

private:
    zerodds_ZeroDdsDataReader* reader_{nullptr};
};

} // namespace cond
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_COND_READCONDITION_HPP
