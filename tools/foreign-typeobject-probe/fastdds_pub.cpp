// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Fast DDS publishes one type of the OMG dds-xtypes suite with its own
// TypeInformation and writes the sample of one of the suite's data files.
//
// The suite has no Fast DDS `test_main`, and Fast DDS's `shape_main` of the
// dds-rtps suite announces no TypeInformation. This publisher builds the
// type with Fast DDS's own IDL parser (`create_type_w_uri`), so the
// TypeObject on the wire is the one Fast DDS derives, and fills the sample
// with Fast DDS's own JSON reader (`json_deserialize`, OMG format).
//
//   fastdds_pub --idl <file.idl> --type <Module::Name> --topic <t>
//               --data <file.json> --domain <n> --xcdr <1|2> [--seconds <s>]
//
// The suite declares one type name in several files with different
// bodies (`Test::enum1` is an enumeration member in `arrays.idl` and a
// sequence of them in `sequences.idl`); a case names its file with
// `--type-file`, and that file is the one given here.
//
// Build: see README.md.

#include <chrono>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>
#include <thread>

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/qos/DataWriterQos.hpp>
#include <fastdds/dds/topic/Topic.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicData.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicPubSubType.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicType.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicTypeBuilder.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicTypeBuilderFactory.hpp>
#include <fastdds/dds/xtypes/utils.hpp>

using namespace eprosima::fastdds::dds;

namespace {

int fail(const std::string& what)
{
    std::cerr << "fastdds_pub: " << what << std::endl;
    return 1;
}

DynamicType::_ref_type load_type(
        const std::string& idl,
        const std::string& type_name)
{
    const auto slash = idl.rfind('/');
    IncludePathSeq include_paths{slash == std::string::npos ? std::string(".") : idl.substr(0, slash)};
    auto builder = DynamicTypeBuilderFactory::get_instance()->create_type_w_uri(idl, type_name, include_paths);
    return builder ? builder->build() : nullptr;
}

} // namespace

int main(
        int argc,
        char** argv)
{
    std::string idl, type_name, topic_name, data_file;
    int domain = -1, xcdr = 0, seconds = 20;
    for (int i = 1; i + 1 < argc; i += 2)
    {
        const std::string flag = argv[i];
        const std::string value = argv[i + 1];
        if (flag == "--idl") idl = value;
        else if (flag == "--type") type_name = value;
        else if (flag == "--topic") topic_name = value;
        else if (flag == "--data") data_file = value;
        else if (flag == "--domain") domain = std::atoi(value.c_str());
        else if (flag == "--xcdr") xcdr = std::atoi(value.c_str());
        else if (flag == "--seconds") seconds = std::atoi(value.c_str());
        else return fail("unknown flag " + flag);
    }
    if (idl.empty() || type_name.empty() || topic_name.empty() || data_file.empty() || domain < 0
            || (xcdr != 1 && xcdr != 2))
    {
        return fail("usage: --idl <file> --type <name> --topic <t> --data <json> --domain <n> --xcdr <1|2>");
    }

    DynamicType::_ref_type type = load_type(idl, type_name);
    if (!type)
    {
        return fail("Fast DDS's IDL parser built no " + type_name + " from " + idl);
    }
    std::ifstream data_in(data_file);
    if (!data_in)
    {
        return fail("cannot read " + data_file);
    }
    std::stringstream json;
    json << data_in.rdbuf();
    DynamicData::_ref_type sample;
    if (json_deserialize(json.str(), type, DynamicDataJsonFormat::OMG, sample) != RETCODE_OK || !sample)
    {
        return fail("json_deserialize refused " + data_file + " for " + type_name);
    }

    DomainParticipant* participant =
            DomainParticipantFactory::get_instance()->create_participant(domain, PARTICIPANT_QOS_DEFAULT);
    if (!participant)
    {
        return fail("create_participant");
    }
    TypeSupport type_support(new DynamicPubSubType(type));
    if (type_support.register_type(participant) != RETCODE_OK)
    {
        return fail("register_type");
    }
    Topic* topic = participant->create_topic(topic_name, type_support.get_type_name(), TOPIC_QOS_DEFAULT);
    Publisher* publisher = participant->create_publisher(PUBLISHER_QOS_DEFAULT);
    if (!topic || !publisher)
    {
        return fail("create_topic/create_publisher");
    }
    DataWriterQos wqos = DATAWRITER_QOS_DEFAULT;
    wqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    wqos.durability().kind = TRANSIENT_LOCAL_DURABILITY_QOS;
    wqos.representation().m_value.clear();
    wqos.representation().m_value.push_back(
        xcdr == 1 ? XCDR_DATA_REPRESENTATION : XCDR2_DATA_REPRESENTATION);
    DataWriter* writer = publisher->create_datawriter(topic, wqos);
    if (!writer)
    {
        return fail("create_datawriter");
    }
    std::cout << "fastdds_pub: " << type_support.get_type_name() << " on " << topic_name << " xcdr" << xcdr
              << std::endl;
    const auto until = std::chrono::steady_clock::now() + std::chrono::seconds(seconds);
    while (std::chrono::steady_clock::now() < until)
    {
        if (writer->write(&sample) != RETCODE_OK)
        {
            return fail("write");
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(200));
    }
    participant->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(participant);
    return 0;
}
