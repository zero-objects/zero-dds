// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/sub/AnyDataReader.hpp — AnyDataReader (DDS-PSM-Cxx 1.0 §7.5.3): a
// DataReader of any topic type, with the operations that do not depend on
// the type, and `get<T>()` to get the typed DataReader back.

#ifndef ZERODDS_DDS_SUB_ANYDATAREADER_HPP
#define ZERODDS_DDS_SUB_ANYDATAREADER_HPP


#include "zerodds/psm/cxx.hpp"
#include <memory>
#include <string>

#include "dds/sub/detail/DataReaderFwd.hpp"
#include "dds/core/Reference.hpp"
#include "dds/core/Duration.hpp"
#include "dds/core/Exception.hpp"
#include "dds/sub/qos/DataReaderQos.hpp"
#include "zerodds.h"

namespace dds {
namespace sub {

class Subscriber;
class AnyDataReader;

namespace detail {

/// What AnyDataReader does without knowing the topic type.
class DRHolderBase {
public:
    virtual ~DRHolderBase() {}
    virtual const ::dds::sub::qos::DataReaderQos& qos() const = 0;
    virtual void qos(const ::dds::sub::qos::DataReaderQos& q) = 0;
    virtual const std::string& topic_name() const = 0;
    virtual const std::string& type_name() const = 0;
    virtual const ::dds::sub::Subscriber& subscriber() const = 0;
    virtual void wait_for_historical_data(const ::dds::core::Duration& timeout) = 0;
    virtual void close() = 0;
    virtual void retain(bool b) = 0;
    /// The C handle of the reader (internal; conditions and queries bind to it).
    virtual zerodds_ZeroDdsDataReader* native_reader() const = 0;
};

template <typename T>
class DRHolder;
class NativeDataReaderHolder;
struct AnyDataReaderAccess;

/// An AnyDataReader for the reader behind a C handle whose type the caller
/// did not name (a listener callback); the handle stays owned by its
/// creator.
AnyDataReader make_native_any_reader(zerodds_ZeroDdsDataReader* raw);

} // namespace detail

class AnyDataReader : public ::dds::core::Reference<detail::DRHolderBase> {
public:
    typedef detail::DRHolderBase DELEGATE_T;
    typedef ::dds::core::Reference<DELEGATE_T> BASE_T;

    /// The null AnyDataReader (`dds::core::null`).
    AnyDataReader(const ::dds::core::null_type&) ZERODDS_NOEXCEPT : BASE_T() {}
    template <typename T>
    AnyDataReader(const DataReader<T>& dr);

    const detail::DRHolderBase* operator->() const { return &holder(); }
    detail::DRHolderBase* operator->() { return &holder(); }

    AnyDataReader& swap(AnyDataReader& rhs) {
        this->impl_.swap(rhs.impl_);
        return *this;
    }
    template <typename T>
    AnyDataReader& operator=(const DataReader<T>& rhs);
    AnyDataReader& operator=(AnyDataReader rhs) {
        this->impl_ = ZERODDS_MOVE(rhs.impl_);
        return *this;
    }

    /// The reader as a `DataReader<T>`; `InvalidDowncastError` if it is of
    /// another type.
    template <typename T>
    DataReader<T> get();

private:
    friend struct detail::AnyDataReaderAccess;

    AnyDataReader() {}

    detail::DRHolderBase& holder() const {
        if (!this->impl_) {
            throw ::dds::core::NullReferenceError("AnyDataReader: null");
        }
        return *this->impl_;
    }

};

/// The reader of `adr` as a `DataReader<T>`.
template <typename T>
DataReader<T> get(const AnyDataReader& adr) {
    AnyDataReader copy = adr;
    return copy.get<T>();
}

} // namespace sub
} // namespace dds

#endif // ZERODDS_DDS_SUB_ANYDATAREADER_HPP
