// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/core/external.hpp — `dds::core::external<T>`, the representation of an
// `@external` member in the C++ binding for DDS-PSM-CXX. DDS-XTypes 1.3
// §7.5.1.2.3.3: "External members shall be represented as an instantiation
// of a template class external<T>, where T is the type of the external
// member. This is a 'smart pointer' class that wraps a shared pointer, ptr_
// for automatic reference counting and a boolean locked_ that controls the
// assignment behavior."
//
// The value of `locked_` dictates whether copying performs a deep or a
// shallow copy and whether the object may be shared (§7.5.1.2.3.3.4,
// §7.5.1.2.3.3.5, Table 46). It is set when the object is constructed and
// can only be true while the shared pointer is not empty.
#ifndef DDS_CORE_EXTERNAL_HPP
#define DDS_CORE_EXTERNAL_HPP

#include <memory>
#include <stdexcept>
#include <utility>

namespace dds {
namespace core {

/// Table 46: "Error. Operation cannot be called when
/// destination.is_locked() == TRUE".
class external_locked_error : public std::logic_error {
  public:
    external_locked_error()
        : std::logic_error("dds::core::external: the destination is locked") {}
};

/// An external<T> with nothing in it was dereferenced, or locked.
class external_empty_error : public std::logic_error {
  public:
    explicit external_empty_error(const char *what_arg) : std::logic_error(what_arg) {}
};

template <typename T>
class external {
  public:
    /// §7.5.1.2.3.3.1: "Create an empty external<T> object with an empty
    /// ptr_ and locked_ initialized to false."
    external() : ptr_(), locked_(false) {}

    /// §7.5.1.2.3.3.2: "Create a new external<T> object referencing the
    /// provided managed object. The attribute locked_ is set to false and
    /// ptr_ is initialized with p." With `locked` true, locked_ is true;
    /// that is only possible for a pointer that is not NULL.
    external(T *p, bool locked = false) : ptr_(p), locked_(locked) {
        if (locked_ && !ptr_) {
            throw external_empty_error(
                "dds::core::external: locked_ can only be true for a non-NULL pointer");
        }
    }

    /// §7.5.1.2.3.3.3: "Create a new external<T> object that references the
    /// same object managed by the specified shared pointer p. The attribute
    /// locked_ is set to false and ptr_ is initialized with p."
    external(std::shared_ptr<T> p) : ptr_(std::move(p)), locked_(false) {}

    /// §7.5.1.2.3.3.4. If `other` is not locked the new object shares the
    /// reference. If it is locked a new T is created and filled with a copy
    /// of the contents of `other` — a deep copy. Either way locked_ is false.
    external(const external &other) : ptr_(), locked_(false) {
        if (other.locked_) {
            ptr_ = std::make_shared<T>(*other.ptr_);
        } else {
            ptr_ = other.ptr_;
        }
    }

    /// §7.5.1.2.3.3.6: "If ptr_ is the last reference to the managed
    /// object, then the managed object will be released, otherwise the
    /// reference count will simply be decreased." The shared pointer does it.
    ~external() = default;

    /// §7.5.1.2.3.3.5 and Table 46.
    ///
    /// | destination locked_ | ptr_      | source locked_ | ptr_      | |
    /// |---|---|---|---|---|
    /// | TRUE  | any       | any   | any       | error |
    /// | FALSE | any       | any   | EMPTY     | the destination is reset |
    /// | FALSE | EMPTY     | TRUE  | not EMPTY | a new object, a deep copy |
    /// | FALSE | not EMPTY | TRUE  | not EMPTY | the existing object, a deep copy |
    /// | FALSE | any       | FALSE | not EMPTY | a shallow copy |
    external &operator=(const external &other) {
        if (locked_) {
            throw external_locked_error();
        }
        if (!other.ptr_) {
            ptr_.reset();
        } else if (other.locked_) {
            if (!ptr_) {
                ptr_ = std::make_shared<T>(*other.ptr_);
            } else {
                *ptr_ = *other.ptr_;
            }
        } else {
            ptr_ = other.ptr_;
        }
        return *this;
    }

    /// §7.5.1.2.3.3.7: a reference to the object `ptr_` points at.
    T &operator*() { return *checked(); }
    const T &operator*() const { return *checked(); }

    /// §7.5.1.2.3.3.8: a pointer to the managed object, NULL when empty.
    T *get() { return ptr_.get(); }
    const T *get() const { return ptr_.get(); }

    /// §7.5.1.2.3.3.9: a shared pointer to the managed object.
    std::shared_ptr<T> get_shared_ptr() { return ptr_; }

    /// §7.5.1.2.3.3.10: access to the members of the managed object.
    T *operator->() { return checked(); }
    const T *operator->() const { return checked(); }

    /// §7.5.1.2.3.3.11: "Returns whether two external objects manage the
    /// same object or are both empty."
    bool operator==(const external<T> &other) const { return ptr_ == other.ptr_; }

    /// §7.5.1.2.3.3.12: "Returns whether two external objects do not manage
    /// the same object."
    bool operator!=(const external<T> &other) const { return !(*this == other); }

    /// §7.5.1.2.3.3.13: "Checks if there is a managed object (is not NULL)
    /// or not (is NULL)."
    operator bool() const { return static_cast<bool>(ptr_); }

    /// §7.5.1.2.3.3.14: whether this object is locked.
    bool is_locked() const { return locked_; }

    /// §7.5.1.2.3.3.15: sets locked_ to true. Future assignments to this
    /// object fail and any copy from it is a deep copy. locked_ can only be
    /// true while there is a managed object.
    void lock() {
        if (!ptr_) {
            throw external_empty_error(
                "dds::core::external: locked_ can only be true for a non-NULL pointer");
        }
        locked_ = true;
    }

  private:
    T *checked() const {
        if (!ptr_) {
            throw external_empty_error("dds::core::external: no managed object");
        }
        return ptr_.get();
    }

    std::shared_ptr<T> ptr_;
    bool locked_;
};

}  // namespace core
}  // namespace dds

#endif  // DDS_CORE_EXTERNAL_HPP
