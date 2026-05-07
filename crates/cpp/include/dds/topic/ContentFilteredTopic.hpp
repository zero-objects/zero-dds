// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// dds/topic/ContentFilteredTopic.hpp — DDS-PSM-Cxx 1.0 §7.5.13.7.

#ifndef ZERODDS_DDS_TOPIC_CONTENTFILTEREDTOPIC_HPP
#define ZERODDS_DDS_TOPIC_CONTENTFILTEREDTOPIC_HPP

#include <string>
#include <vector>

#include "dds/core/Exception.hpp"
#include "dds/topic/Topic.hpp"
#include "zerodds.h"

namespace dds {
namespace topic {

/// `ContentFilteredTopic<T>` (Spec §7.5.13.7).
template <typename T>
class ContentFilteredTopic : public TopicDescription {
public:
    /// Konstruiert ueber Related-Topic + Filter-Expression + Parameter.
    ContentFilteredTopic(::dds::domain::DomainParticipant& dp,
                         const std::string& name, Topic<T>& related_topic,
                         const std::string& filter_expression,
                         const std::vector<std::string>& parameters)
        : TopicDescription(nullptr), participant_(dp.native_handle()) {
        std::vector<const char*> raw_params;
        raw_params.reserve(parameters.size());
        for (const auto& p : parameters) {
            raw_params.push_back(p.c_str());
        }
        cft_handle_ = zerodds_dp_create_contentfilteredtopic(
            participant_, name.c_str(), related_topic.native_handle(),
            filter_expression.c_str(),
            raw_params.empty() ? nullptr : raw_params.data(),
            raw_params.size());
        if (cft_handle_ == nullptr) {
            throw ::dds::core::Error("ContentFilteredTopic::create failed");
        }
        related_topic_ = related_topic.native_handle();
        // Im RC1: TopicDescription::handle_ bleibt der Topic-Pointer, da
        // CFT auf C-FFI-Ebene als opaque Sub-Type ist und in DataReader-
        // create-Pfad als TopicDescription-Pointer durchgereicht wird.
        // (Folge-Wave: richtige Reader-Bind via cft_handle_.)
        handle_ = related_topic.native_handle();
    }

    ContentFilteredTopic(const ContentFilteredTopic&) = delete;
    ContentFilteredTopic& operator=(const ContentFilteredTopic&) = delete;
    ContentFilteredTopic(ContentFilteredTopic&& o) noexcept
        : TopicDescription(o.handle_),
          participant_(o.participant_),
          cft_handle_(o.cft_handle_),
          related_topic_(o.related_topic_) {
        o.cft_handle_ = nullptr;
    }
    ContentFilteredTopic& operator=(ContentFilteredTopic&& o) noexcept {
        if (this != &o) {
            close();
            handle_ = o.handle_;
            participant_ = o.participant_;
            cft_handle_ = o.cft_handle_;
            related_topic_ = o.related_topic_;
            o.cft_handle_ = nullptr;
        }
        return *this;
    }
    ~ContentFilteredTopic() { close(); }

private:
    void close() {
        if (cft_handle_ != nullptr && participant_ != nullptr) {
            zerodds_dp_delete_contentfilteredtopic(participant_, cft_handle_);
            cft_handle_ = nullptr;
        }
    }
    zerodds_ZeroDdsDomainParticipant* participant_{nullptr};
    zerodds_ZeroDdsContentFilteredTopic* cft_handle_{nullptr};
    zerodds_ZeroDdsTopic* related_topic_{nullptr};
};

} // namespace topic
} // namespace dds

#endif // ZERODDS_DDS_TOPIC_CONTENTFILTEREDTOPIC_HPP
