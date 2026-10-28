// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/cond/detail/ReadConditionDelegate.hpp — delegates of ReadCondition
// and QueryCondition (DDS-PSM-Cxx 1.0 §7.5, §7.5).

#ifndef ZERODDS_DDS_SUB_COND_DETAIL_READCONDITIONDELEGATE_HPP
#define ZERODDS_DDS_SUB_COND_DETAIL_READCONDITIONDELEGATE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <functional>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/Exception.hpp"
#include "zerodds/psm/check_status.hpp"
#include "dds/core/cond/detail/ConditionDelegate.hpp"
#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/status/DataState.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {
namespace cond {

namespace detail {

/// What a ReadCondition holds: the reader, the state it waits for, the
/// functor to dispatch.
class ReadConditionDelegate : public ::dds::core::cond::detail::ConditionDelegate {
public:
    /// Called by `dispatch()` with the delegate of the condition.
    typedef ::zerodds::cxx::function<void(const ::zerodds::cxx::shared_ptr<ReadConditionDelegate>&)> Functor;

    ReadConditionDelegate(const ::dds::sub::AnyDataReader& reader,
                          const ::dds::sub::status::DataState& state, Functor functor)
        : ConditionDelegate(zerodds_dr_create_readcondition(
              reader->native_reader(), mask(state.sample_state()), mask(state.view_state()),
              mask(state.instance_state()))),
          reader_(reader),
          state_(state),
          functor_(ZERODDS_MOVE(functor)) {
        if (handle_ == 0) {
            throw ::dds::core::Error("ReadCondition: the C API refused to create the condition");
        }
    }
    ~ReadConditionDelegate() ZERODDS_OVERRIDE { release(); }

    ZERODDS_NONCOPYABLE(ReadConditionDelegate)
#if ZERODDS_CXX11
    ReadConditionDelegate(ReadConditionDelegate&&) = delete;
    ReadConditionDelegate& operator=(ReadConditionDelegate&&) = delete;
#endif

    const ::dds::sub::status::DataState& state_filter() const { return state_; }
    const ::dds::sub::AnyDataReader& data_reader() const { return reader_; }

    /// Calls the functor when the condition holds.
    void dispatch() ZERODDS_OVERRIDE {
        if (functor_ && trigger_value()) {
            ::zerodds::cxx::shared_ptr<ReadConditionDelegate> me = self_.lock();
            if (me) {
                functor_(me);
            }
        }
    }

    /// The condition's own weak reference, set by the class that owns the delegate.
    void bind(const ::zerodds::cxx::shared_ptr<ReadConditionDelegate>& self) { self_ = self; }

protected:
    /// For QueryConditionDelegate: the native condition comes from another call.
    ReadConditionDelegate(const ::dds::sub::AnyDataReader& reader,
                          const ::dds::sub::status::DataState& state, Functor functor,
                          void* native)
        : ConditionDelegate(native), reader_(reader), state_(state), functor_(ZERODDS_MOVE(functor)) {
        if (handle_ == 0) {
            throw ::dds::core::Error("QueryCondition: the C API refused to create the condition");
        }
    }

    void release() {
        if (handle_ != 0) {
            zerodds_dr_delete_readcondition(reader_->native_reader(), handle_);
            handle_ = 0;
        }
    }

    template <typename Bits>
    static uint32_t mask(const Bits& bits) {
        return static_cast<uint32_t>(bits.to_ulong());
    }

private:
    ::dds::sub::AnyDataReader reader_;
    ::dds::sub::status::DataState state_;
    Functor functor_;
    ::zerodds::cxx::weak_ptr<ReadConditionDelegate> self_;
};

/// What a QueryCondition holds beside a ReadCondition's: the expression and
/// its parameters.
class QueryConditionDelegate : public ReadConditionDelegate {
public:
    typedef std::vector<std::string>::iterator iterator;
    typedef std::vector<std::string>::const_iterator const_iterator;

    QueryConditionDelegate(const ::dds::sub::AnyDataReader& reader, const std::string& expression,
                           const std::vector<std::string>& parameters,
                           const ::dds::sub::status::DataState& state, Functor functor)
        : ReadConditionDelegate(reader, state, ZERODDS_MOVE(functor),
                                create(reader, expression, parameters, state)),
          expression_(expression),
          parameters_(parameters) {}

    const std::string& expression() const { return expression_; }
    iterator begin() { return parameters_.begin(); }
    iterator end() { return parameters_.end(); }
    const_iterator begin() const { return parameters_.begin(); }
    const_iterator end() const { return parameters_.end(); }
    uint32_t parameters_length() const { return static_cast<uint32_t>(parameters_.size()); }

    /// DDS-PSM-Cxx 1.0 `QueryCondition::expression(str)`.
    void expression(const std::string& expr) {
        ::zerodds::psm::check_status(zerodds_querycondition_set_expression(handle_, expr.c_str()),
                                     "QueryCondition::expression");
        expression_ = expr;
    }

    /// DDS 1.4 §2.2.2.5.9 `set_query_parameters`.
    void parameters(const std::vector<std::string>& params) {
        std::vector<const char*> raw;
        raw.reserve(params.size());
        for (std::vector<std::string>::const_iterator p = params.begin(); p != params.end(); ++p) {
            raw.push_back(p->c_str());
        }
        ::zerodds::psm::check_status(
            zerodds_querycondition_set_parameters(handle_, raw.empty() ? 0 : &raw[0], raw.size()),
            "QueryCondition::parameters");
        parameters_ = params;
    }

private:
    static void* create(const ::dds::sub::AnyDataReader& reader, const std::string& expression,
                        const std::vector<std::string>& parameters,
                        const ::dds::sub::status::DataState& state) {
        std::vector<const char*> raw;
        raw.reserve(parameters.size());
        for (std::vector<std::string>::const_iterator p = parameters.begin(); p != parameters.end();
             ++p) {
            raw.push_back(p->c_str());
        }
        return zerodds_dr_create_querycondition(
            reader->native_reader(), mask(state.sample_state()), mask(state.view_state()),
            mask(state.instance_state()), expression.c_str(), raw.empty() ? 0 : &raw[0],
            raw.size());
    }

    std::string expression_;
    std::vector<std::string> parameters_;
};

} // namespace detail
} // namespace cond
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_COND_DETAIL_READCONDITIONDELEGATE_HPP
