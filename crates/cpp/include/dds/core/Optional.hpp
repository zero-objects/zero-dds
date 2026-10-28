// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/Optional.hpp — DDS-PSM-Cxx 1.0 §8.1.4: `dds::core::optional<T>`,
// the mapping of an IDL member annotated `@Optional`.

#ifndef ZERODDS_DDS_CORE_OPTIONAL_HPP
#define ZERODDS_DDS_CORE_OPTIONAL_HPP


#include "zerodds/psm/cxx.hpp"
#include <new>
#include <utility>

// std::optional exists from C++17; before that, a small own storage with the same members.
#if (defined(__cplusplus) && __cplusplus >= 201703L) || (defined(_MSVC_LANG) && _MSVC_LANG >= 201703L)
#define ZERODDS_OPTIONAL_STD 1
#include <optional>
#else
#define ZERODDS_OPTIONAL_STD 0
#endif

#include "dds/core/Exception.hpp"
#include "dds/core/Value.hpp"

namespace dds {
namespace core {
namespace detail {

#if ZERODDS_OPTIONAL_STD

/// The value slot of `optional_delegate`: `std::optional`.
template <typename T>
class optional_slot {
public:
    optional_slot() {}
    explicit optional_slot(const T& t) : v_(t) {}
    bool has_value() const { return v_.has_value(); }
    void reset() { v_.reset(); }
    T& operator*() { return *v_; }
    const T& operator*() const { return *v_; }
    bool equals(const optional_slot& other) const { return v_ == other.v_; }

private:
    std::optional<T> v_;
};

#else

/// The value slot of `optional_delegate` for a compiler before C++17: in-place storage, the
/// value is constructed and destroyed on demand (no default constructor of `T` needed).
template <typename T>
class optional_slot {
public:
    optional_slot() : set_(false) {}
    explicit optional_slot(const T& t) : set_(false) {
        ::new (static_cast<void*>(storage_.buf)) T(t);
        set_ = true;
    }
    optional_slot(const optional_slot& other) : set_(false) {
        if (other.set_) {
            ::new (static_cast<void*>(storage_.buf)) T(*other);
            set_ = true;
        }
    }
    optional_slot& operator=(const optional_slot& other) {
        if (this == &other) {
            return *this;
        }
        if (other.set_) {
            if (set_) {
                **this = *other;
            } else {
                ::new (static_cast<void*>(storage_.buf)) T(*other);
                set_ = true;
            }
        } else {
            reset();
        }
        return *this;
    }
    ~optional_slot() { reset(); }

    bool has_value() const { return set_; }
    void reset() {
        if (set_) {
            set_ = false;
            ptr()->~T();
        }
    }
    T& operator*() { return *ptr(); }
    const T& operator*() const { return *ptr(); }
    bool equals(const optional_slot& other) const {
        if (set_ != other.set_) {
            return false;
        }
        return !set_ || **this == *other;
    }

private:
    T* ptr() { return reinterpret_cast<T*>(storage_.buf); }
    const T* ptr() const { return reinterpret_cast<const T*>(storage_.buf); }

    union Storage {
        char buf[sizeof(T)];
        long long align_ll;
        long double align_ld;
        double align_d;
        void* align_p;
    };
    Storage storage_;
    bool set_;
};

#endif

/// Storage of `dds::core::optional<T>`.
template <typename T>
class optional_delegate {
public:
    optional_delegate() {}
    optional_delegate(const T& t) : value_(t) {}

    bool is_set() const { return value_.has_value(); }
    void reset() { value_.reset(); }
    const T& get() const { return checked(); }
    T& get() { return checked(); }

    bool operator==(const optional_delegate& other) const { return value_.equals(other.value_); }
    bool operator!=(const optional_delegate& other) const { return !(*this == other); }

private:
    T& checked() {
        if (!value_.has_value()) {
            throw ::dds::core::PreconditionNotMetError("optional::get: no value is set");
        }
        return *value_;
    }
    const T& checked() const {
        if (!value_.has_value()) {
            throw ::dds::core::PreconditionNotMetError("optional::get: no value is set");
        }
        return *value_;
    }

    optional_slot<T> value_;
};

} // namespace detail

/// A value of type `T` that may be absent.
template <typename T, template <typename Q> class DELEGATE = detail::optional_delegate>
class optional : public ::dds::core::Value<DELEGATE<T> > {
public:
    /// An absent value.
    optional() : ::dds::core::Value<DELEGATE<T> >() {}

    /// A present value.
    optional(const T& t) : ::dds::core::Value<DELEGATE<T> >(t) {}

    /// Whether a value is set.
    bool is_set() const { return this->delegate().is_set(); }

    /// Makes the value absent.
    void reset() { this->delegate().reset(); }

    /// The value; throws `PreconditionNotMetError` when none is set.
    const T& get() const { return this->delegate().get(); }
    T& get() { return this->delegate().get(); }
};

} // namespace core
} // namespace dds

#endif // ZERODDS_DDS_CORE_OPTIONAL_HPP
