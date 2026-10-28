// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/Filter.hpp — Filter (DDS-PSM-Cxx 1.0 §7.8): the content filter
// expression of a ContentFilteredTopic with its parameters.

#ifndef ZERODDS_DDS_TOPIC_FILTER_HPP
#define ZERODDS_DDS_TOPIC_FILTER_HPP


#include "zerodds/psm/cxx.hpp"
#include <stdint.h>

#include <stdint.h>
#include <string>
#include <vector>

#include "dds/topic/TFilter.hpp"
#include "zerodds/psm/expression_parameters.hpp"

namespace dds {
namespace topic {

namespace detail {

/// The delegate of TFilter: the expression and its parameters.
class Filter : public ::zerodds::psm::ExpressionParameters {
public:
    Filter(const std::string& query_expression)
        : ::zerodds::psm::ExpressionParameters(query_expression) {}
    template <typename FWIterator>
    Filter(const std::string& query_expression, const FWIterator& params_begin,
           const FWIterator& params_end)
        : ::zerodds::psm::ExpressionParameters(query_expression, params_begin, params_end) {}
    Filter(const std::string& query_expression, const std::vector<std::string>& params)
        : ::zerodds::psm::ExpressionParameters(query_expression, params) {}
};

} // namespace detail

typedef TFilter<detail::Filter> Filter;

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_FILTER_HPP
