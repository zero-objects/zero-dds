// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/TFilter.hpp — DDS-PSM-Cxx 1.0 `TFilter<D>`: a content filter expression with its parameters.

#ifndef ZERODDS_DDS_TOPIC_TFILTER_HPP
#define ZERODDS_DDS_TOPIC_TFILTER_HPP

#include <stdint.h>

#include <string>
#include <vector>

#include "dds/core/Value.hpp"

namespace dds {
namespace topic {

/// A filter expression with parameters, over any delegate.
template <typename D>
class TFilter : public ::dds::core::Value<D> {
public:
    typedef typename D::iterator iterator;
    typedef typename D::const_iterator const_iterator;

    TFilter(const std::string& query_expression) : ::dds::core::Value<D>(query_expression) {}

    template <typename FWIterator>
    TFilter(const std::string& query_expression, const FWIterator& params_begin,
            const FWIterator& params_end)
        : ::dds::core::Value<D>(D(query_expression, params_begin, params_end)) {}

    TFilter(const std::string& query_expression, const std::vector<std::string>& params)
        : ::dds::core::Value<D>(query_expression, params) {}

    const std::string& expression() const { return this->d_.expression(); }
    const_iterator begin() const { return this->d_.begin(); }
    const_iterator end() const { return this->d_.end(); }
    iterator begin() { return this->d_.begin(); }
    iterator end() { return this->d_.end(); }

    template <typename FWIterator>
    void parameters(const FWIterator& begin, const FWIterator end) {
        this->d_.parameters(begin, end);
    }
    void add_parameter(const std::string& param) { this->d_.add_parameter(param); }
    uint32_t parameters_length() const { return this->d_.parameters_length(); }
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_TFILTER_HPP
