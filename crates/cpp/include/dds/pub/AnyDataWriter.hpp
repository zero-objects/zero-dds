// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/AnyDataWriter.hpp — AnyDataWriter (DDS-PSM-Cxx 1.0 §7.5.3): a
// DataWriter of any topic type, with the operations that do not depend on
// the type, and `get<T>()` to get the typed DataWriter back.

#ifndef ZERODDS_DDS_PUB_ANYDATAWRITER_HPP
#define ZERODDS_DDS_PUB_ANYDATAWRITER_HPP


#include "zerodds/psm/cxx.hpp"
#include <string>
#include <typeinfo>

#include "dds/core/Reference.hpp"
#include "dds/core/Duration.hpp"
#include "dds/core/Exception.hpp"
#include "dds/pub/qos/DataWriterQos.hpp"
#include "dds/pub/detail/DataWriterFwd.hpp"
#include "zerodds.h"

namespace dds {
namespace pub {

class Publisher;
class AnyDataWriter;

namespace detail {

/// What AnyDataWriter does without knowing the topic type.
class DWHolderBase {
public:
    virtual ~DWHolderBase() {}
    virtual const ::dds::pub::qos::DataWriterQos& qos() const = 0;
    virtual void qos(const ::dds::pub::qos::DataWriterQos& q) = 0;
    virtual const std::string& topic_name() const = 0;
    virtual const std::string& type_name() const = 0;
    virtual const ::dds::pub::Publisher& publisher() const = 0;
    virtual void wait_for_acknowledgments(const ::dds::core::Duration& timeout) = 0;
    virtual void close() = 0;
    virtual void retain(bool b) = 0;
};

template <typename T>
class DWHolder;
class NativeDataWriterHolder;
struct AnyDataWriterAccess;

/// An AnyDataWriter for the writer behind a C handle whose type the caller
/// did not name (a listener callback); the handle stays owned by its
/// creator.
AnyDataWriter make_native_any_writer(zerodds_ZeroDdsDataWriter* raw);

} // namespace detail

class AnyDataWriter : public ::dds::core::Reference<detail::DWHolderBase> {
public:
    typedef detail::DWHolderBase DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE_T> BASE_T;

    /// The null AnyDataWriter (`dds::core::null`).
    AnyDataWriter(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T() {}
    template <typename T>
    AnyDataWriter(const DataWriter<T>& dw);

    const ::dds::pub::qos::DataWriterQos& qos() const { return holder().qos(); }
    void qos(const ::dds::pub::qos::DataWriterQos& q) { holder().qos(q); }
    const std::string& topic_name() const { return holder().topic_name(); }
    const std::string& type_name() const { return holder().type_name(); }
    const ::dds::pub::Publisher& publisher() const { return holder().publisher(); }
    void wait_for_acknowledgments(const ::dds::core::Duration& timeout) {
        holder().wait_for_acknowledgments(timeout);
    }
    void close() { holder().close(); }
    void retain(bool b) { holder().retain(b); }

    AnyDataWriter& swap(AnyDataWriter& rhs) {
        this->impl_.swap(rhs.impl_);
        return *this;
    }
    template <typename T>
    AnyDataWriter& operator=(const DataWriter<T>& rhs);
    AnyDataWriter& operator=(AnyDataWriter rhs) {
        this->impl_ = ZERODDS_MOVE(rhs.impl_);
        return *this;
    }

    /// The writer as a `DataWriter<T>`; `InvalidDowncastError` if it is of
    /// another type.
    template <typename T>
    DataWriter<T> get();

    const detail::DWHolderBase* operator->() const { return &holder(); }
    detail::DWHolderBase* operator->() { return &holder(); }

private:
    friend struct detail::AnyDataWriterAccess;

    AnyDataWriter() {}

    detail::DWHolderBase& holder() const {
        if (!this->impl_) {
            throw ::dds::core::NullReferenceError("AnyDataWriter: null");
        }
        return *this->impl_;
    }

};

/// The writer of `adw` as a `DataWriter<T>`.
template <typename T>
DataWriter<T> get(const AnyDataWriter& adw) {
    AnyDataWriter copy = adw;
    return copy.get<T>();
}

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_ANYDATAWRITER_HPP
