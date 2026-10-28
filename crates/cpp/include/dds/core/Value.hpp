// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Value.hpp — DDS-PSM-Cxx 1.0 §7.5.1, §7.6.1.

#ifndef ZERODDS_DDS_CORE_VALUE_HPP
#define ZERODDS_DDS_CORE_VALUE_HPP


#include "zerodds/psm/cxx.hpp"
#include <utility>

#include "dds/core/types.hpp"

namespace dds {
namespace core {

/// Base class for all value types (DDS-PSM-Cxx 1.0 §7.6.1).
/// A Value type holds its delegate by value and has copy semantics.
template <typename D>
class Value {
protected:
    Value() {}
#if ZERODDS_CXX11
    Value(const Value& other) = default;
    Value(Value&& other) = default;
#endif

public:
    /// Construct from delegate arguments (perfect forwarding for up to 6 args).
    template <typename ARG>
    explicit Value(const ARG& arg) : d_(arg) {}

    template <typename ARG1, typename ARG2>
    Value(const ARG1& arg1, const ARG2& arg2) : d_(arg1, arg2) {}

    template <typename ARG1, typename ARG2, typename ARG3>
    Value(const ARG1& arg1, const ARG2& arg2, const ARG3& arg3)
        : d_(arg1, arg2, arg3) {}

    template <typename ARG1, typename ARG2, typename ARG3, typename ARG4>
    Value(const ARG1& arg1, const ARG2& arg2, const ARG3& arg3, const ARG4& arg4)
        : d_(arg1, arg2, arg3, arg4) {}

    template <typename ARG1, typename ARG2, typename ARG3, typename ARG4, typename ARG5>
    Value(const ARG1& arg1, const ARG2& arg2, const ARG3& arg3, const ARG4& arg4,
          const ARG5& arg5)
        : d_(arg1, arg2, arg3, arg4, arg5) {}

    template <typename ARG1, typename ARG2, typename ARG3, typename ARG4, typename ARG5,
              typename ARG6>
    Value(const ARG1& arg1, const ARG2& arg2, const ARG3& arg3, const ARG4& arg4,
          const ARG5& arg5, const ARG6& arg6)
        : d_(arg1, arg2, arg3, arg4, arg5, arg6) {}

#if ZERODDS_CXX11
    /// Takes the delegate by move.
    explicit Value(D&& d) : d_(ZERODDS_MOVE(d)) {}
#endif

#if ZERODDS_CXX11
    ~Value() = default;
#endif

#if ZERODDS_CXX11
    Value& operator=(Value&& other) {
        if (this != &other) {
            d_ = ZERODDS_MOVE(other.d_);
        }
        return *this;
    }
#endif
    Value& operator=(const Value& other) {
        if (this != &other) {
            d_ = other.d_;
        }
        return *this;
    }

    bool operator==(const Value& other) const { return d_ == other.d_; }
    bool operator!=(const Value& other) const { return d_ != other.d_; }

    /// Direct delegate member access.
    const D* operator->() const ZERODDS_NOEXCEPT { return &d_; }
    D* operator->() ZERODDS_NOEXCEPT { return &d_; }

    /// Access the underlying delegate.
    const D& delegate() const ZERODDS_NOEXCEPT { return d_; }
    D& delegate() ZERODDS_NOEXCEPT { return d_; }

    /// Implicit conversion to delegate reference.
    operator D& () ZERODDS_NOEXCEPT { return d_; }
    operator const D& () const ZERODDS_NOEXCEPT { return d_; }

protected:
    D d_;
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_VALUE_HPP