// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/cond/ReadCondition.hpp — DDS-PSM-Cxx 1.0 §7.5
// `dds::sub::cond::ReadCondition` and §7.5 `QueryCondition`.

#ifndef ZERODDS_DDS_SUB_COND_READCONDITION_HPP
#define ZERODDS_DDS_SUB_COND_READCONDITION_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <memory>
#include <cstddef>
#include <string>
#include <utility>
#include <vector>

#include "dds/core/cond/TCondition.hpp"
#include "dds/sub/AnyDataReader.hpp"
#include "dds/sub/Query.hpp"
#include "dds/sub/cond/TReadCondition.hpp"
#include "dds/sub/cond/detail/ReadConditionDelegate.hpp"
#include "dds/sub/DataReader.hpp"
#include "dds/sub/status/DataState.hpp"

namespace dds {
namespace sub {
namespace cond {
namespace detail {

/// The functor of a `Cond` (`ReadCondition` or `QueryCondition`): `f` is called
/// with the condition when it can take one, and without otherwise.
/// Whether a `const FUN&` can be called with a `const Cond&` (ISO C++03 has no `is_invocable`:
/// the call is put into a `sizeof`, and a substitution failure removes the overload).
template <typename Cond, typename FUN>
struct accepts_condition {
    typedef char yes_type;
    struct no_type {
        char pad[2];
    };
    template <std::size_t N>
    struct size_tag {};
    template <typename X>
    static const X& make();
    template <typename F, typename C>
    static yes_type test(size_tag<sizeof((void)make<F>()(make<C>()), 0)>*);
    template <typename F, typename C>
    static no_type test(...);
    static const bool value = sizeof(test<FUN, Cond>(0)) == sizeof(yes_type);
};
template <typename Cond, typename FUN>
const bool accepts_condition<Cond, FUN>::value;

/// The functor of a `Cond`: `f` is called with the condition when it can take one.
template <typename Cond, typename FUN, bool TAKES_CONDITION = accepts_condition<Cond, FUN>::value>
class ConditionFunctor {
public:
    explicit ConditionFunctor(const FUN& f) : f_(f) {}
    void operator()(const ::zerodds::cxx::shared_ptr<ReadConditionDelegate>& me) const {
        f_(Cond(::zerodds::cxx::static_pointer_cast<typename Cond::DELEGATE_T>(me)));
    }

private:
    FUN f_;
};
/// ... and without argument otherwise.
template <typename Cond, typename FUN>
class ConditionFunctor<Cond, FUN, false> {
public:
    explicit ConditionFunctor(const FUN& f) : f_(f) {}
    void operator()(const ::zerodds::cxx::shared_ptr<ReadConditionDelegate>&) const { f_(); }

private:
    FUN f_;
};

template <typename Cond, typename FUN>
ReadConditionDelegate::Functor make_functor(const FUN& f) {
    return ReadConditionDelegate::Functor(ConditionFunctor<Cond, FUN>(f));
}

} // namespace detail

/// Waits for samples in a state. With a functor, `dispatch()` calls it when
/// the condition holds: with the condition as argument if the functor takes
/// one, without otherwise.
class ReadCondition : public TReadCondition<detail::ReadConditionDelegate> {
public:
    typedef TReadCondition<detail::ReadConditionDelegate> BASE_T;
    typedef detail::ReadConditionDelegate DELEGATE_T;

    template <typename T>
    ReadCondition(const ::dds::sub::DataReader<T>& dr, const ::dds::sub::status::DataState& status)
        : BASE_T(make(AnyDataReader(dr), status, DELEGATE_T::Functor())) {}

    template <typename T, typename FUN>
    ReadCondition(const ::dds::sub::DataReader<T>& dr, const ::dds::sub::status::DataState& status,
                  const FUN& functor)
        : BASE_T(make(AnyDataReader(dr), status, detail::make_functor<ReadCondition>(functor))) {}

    /// A condition sharing `ref`.
    explicit ReadCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// The null ReadCondition (`dds::core::null`).
    ReadCondition(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    ReadCondition(const ReadCondition&) = default;
    ReadCondition& operator=(const ReadCondition&) = default;
    ReadCondition(ReadCondition&&) = default;
    ReadCondition& operator=(ReadCondition&&) = default;
    ~ReadCondition() = default;
#endif

private:
    static ::zerodds::cxx::shared_ptr<DELEGATE_T> make(const AnyDataReader& reader,
                                            const ::dds::sub::status::DataState& status,
                                            DELEGATE_T::Functor functor) {
        ::zerodds::cxx::shared_ptr<DELEGATE_T> d =
            ::zerodds::cxx::make_shared<DELEGATE_T>(reader, status, ZERODDS_MOVE(functor));
        d->bind(d);
        return d;
    }
};

/// A ReadCondition that also filters by a query (content-subscription profile).
class QueryCondition : public TReadCondition<detail::QueryConditionDelegate> {
public:
    typedef TReadCondition<detail::QueryConditionDelegate> BASE_T;
    typedef detail::QueryConditionDelegate DELEGATE_T;
    typedef DELEGATE_T::iterator iterator;
    typedef DELEGATE_T::const_iterator const_iterator;

    QueryCondition(const ::dds::sub::Query& query, const ::dds::sub::status::DataState& status)
        : BASE_T(make(query, status, DELEGATE_T::Functor())) {}

    template <typename FUN>
    QueryCondition(const ::dds::sub::Query& query, const ::dds::sub::status::DataState& status,
                   const FUN& functor)
        : BASE_T(make(query, status, detail::make_functor<QueryCondition>(functor))) {}

    /// A condition sharing `ref`.
    explicit QueryCondition(const DELEGATE_REF_T& ref) : BASE_T(ref) {}

    /// The null QueryCondition (`dds::core::null`).
    QueryCondition(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T(DELEGATE_REF_T()) {}

#if ZERODDS_CXX11
    QueryCondition(const QueryCondition&) = default;
    QueryCondition& operator=(const QueryCondition&) = default;
    QueryCondition(QueryCondition&&) = default;
    QueryCondition& operator=(QueryCondition&&) = default;
    ~QueryCondition() = default;
#endif

    /// The filter expression.
    const std::string& expression() const { return this->delegate()->expression(); }

    const_iterator begin() const { return this->delegate()->begin(); }
    const_iterator end() const { return this->delegate()->end(); }
    iterator begin() { return this->delegate()->begin(); }
    iterator end() { return this->delegate()->end(); }
    uint32_t parameters_length() const { return this->delegate()->parameters_length(); }

    /// Replaces the filter expression.
    void expression(const std::string& expr) { this->delegate()->expression(expr); }

    /// Replaces the parameters with `[begin, end)` (DDS 1.4 `set_query_parameters`).
    template <typename FWIterator>
    void parameters(const FWIterator& begin, const FWIterator end) {
        this->delegate()->parameters(std::vector<std::string>(begin, end));
    }

    /// Appends one parameter.
    void add_parameter(const std::string& param) {
        std::vector<std::string> next(begin(), end());
        next.push_back(param);
        this->delegate()->parameters(next);
    }

private:
    static ::zerodds::cxx::shared_ptr<DELEGATE_T> make(const ::dds::sub::Query& query,
                                            const ::dds::sub::status::DataState& status,
                                            DELEGATE_T::Functor functor) {
        std::vector<std::string> params(query.begin(), query.end());
        ::zerodds::cxx::shared_ptr<DELEGATE_T> d = ::zerodds::cxx::make_shared<DELEGATE_T>(
            query.data_reader(), query.expression(), params, status, ZERODDS_MOVE(functor));
        d->bind(d);
        return d;
    }
};

} // namespace cond
} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_COND_READCONDITION_HPP
