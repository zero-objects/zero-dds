// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/pub/detail/DataWriterFwd.hpp — declaration of `dds::pub::DataWriter` with its default
// delegate: `template <typename T, template <typename Q> class DELEGATE = detail::DataWriter>
// class DataWriter` of DDS-PSM-Cxx 1.0 (dds/pub/DataWriter.hpp).

#ifndef ZERODDS_DDS_PUB_DETAIL_DATAWRITERFWD_HPP
#define ZERODDS_DDS_PUB_DETAIL_DATAWRITERFWD_HPP

namespace dds {
namespace pub {
namespace detail {
template <typename T>
class DataWriterDelegate;
} // namespace detail

template <typename T, template <typename Q> class DELEGATE = detail::DataWriterDelegate>
class DataWriter;

} // namespace pub
} // namespace dds

#endif // ZERODDS_DDS_PUB_DETAIL_DATAWRITERFWD_HPP
