// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/TInstanceHandle.hpp — DDS-PSM-Cxx 1.0 §7.5.4 `TInstanceHandle<D>`.

#ifndef ZERODDS_DDS_CORE_TINSTANCEHANDLE_HPP
#define ZERODDS_DDS_CORE_TINSTANCEHANDLE_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include "dds/core/Value.hpp"
#include "dds/core/types.hpp"

namespace dds {
namespace core {

/// A handle to a DDS data instance, over any delegate.
template <typename D>
class TInstanceHandle : public ::dds::core::Value<D> {
public:
    /// HANDLE_NIL.
    TInstanceHandle() : ::dds::core::Value<D>() {}
    TInstanceHandle(const ::dds::core::null_type&) : ::dds::core::Value<D>() {}
    /// From the raw 64-bit handle.
    explicit TInstanceHandle(uint64_t v) : ::dds::core::Value<D>(v) {}
#if ZERODDS_CXX11
    TInstanceHandle(const TInstanceHandle& other) = default;
    TInstanceHandle(TInstanceHandle&&) = default;
    TInstanceHandle& operator=(const TInstanceHandle& that) = default;
    TInstanceHandle& operator=(TInstanceHandle&&) = default;
    ~TInstanceHandle() = default;
#endif

    bool operator==(const TInstanceHandle& that) const { return this->d_ == that.d_; }
    bool operator!=(const TInstanceHandle& that) const { return !(*this == that); }
    bool operator<(const TInstanceHandle& that) const { return this->d_ < that.d_; }

    /// HANDLE_NIL.
    static const TInstanceHandle nil() { return TInstanceHandle(); }
    bool is_nil() const { return this->d_.is_nil(); }

    /// The raw 64-bit handle.
    uint64_t value() const { return this->d_.value(); }
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_TINSTANCEHANDLE_HPP
