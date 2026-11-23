// A Fast DDS participant for crates/websocket-bridge/tests/cross_vendor.rs
// (zerodds-ws-bridge-1.0 §10, §12.3), with the same command line as
// cyclone_reading.c: `sub DOMAIN SECONDS` prints every sample of the topic
// SensorReading (type Sensor::Reading) it takes as one JSON line on stdout;
// `pub DOMAIN ID VALUE UNIT` writes one sample once a reader matched. The
// type is reading.idl, built with the DynamicTypes API of Fast DDS 3
// (DDS-XTypes 1.3 §7.5.2); both endpoints use XCDR2.
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>

#include <fastdds/dds/core/policy/QosPolicies.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/topic/Topic.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicData.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicDataFactory.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicPubSubType.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicTypeBuilder.hpp>
#include <fastdds/dds/xtypes/dynamic_types/DynamicTypeBuilderFactory.hpp>
#include <fastdds/dds/xtypes/dynamic_types/MemberDescriptor.hpp>
#include <fastdds/dds/xtypes/dynamic_types/TypeDescriptor.hpp>

using namespace eprosima::fastdds::dds;

// @appendable struct Sensor::Reading { @key long id; double value; string unit; };
static DynamicType::_ref_type reading_type()
{
    DynamicTypeBuilderFactory::_ref_type factory = DynamicTypeBuilderFactory::get_instance();
    TypeDescriptor::_ref_type td {traits<TypeDescriptor>::make_shared()};
    td->kind(TK_STRUCTURE);
    td->name("Sensor::Reading");
    td->extensibility_kind(ExtensibilityKind::APPENDABLE);
    DynamicTypeBuilder::_ref_type builder {factory->create_type(td)};

    MemberDescriptor::_ref_type id {traits<MemberDescriptor>::make_shared()};
    id->name("id");
    id->type(factory->get_primitive_type(TK_INT32));
    id->is_key(true);
    builder->add_member(id);

    MemberDescriptor::_ref_type value {traits<MemberDescriptor>::make_shared()};
    value->name("value");
    value->type(factory->get_primitive_type(TK_FLOAT64));
    builder->add_member(value);

    MemberDescriptor::_ref_type unit {traits<MemberDescriptor>::make_shared()};
    unit->name("unit");
    unit->type(factory->create_string_type(static_cast<uint32_t>(LENGTH_UNLIMITED))->build());
    builder->add_member(unit);

    return builder->build();
}

static int sub(DomainParticipant* participant, Topic* topic, DynamicType::_ref_type type, int seconds)
{
    Subscriber* subscriber = participant->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    DataReaderQos qos = DATAREADER_QOS_DEFAULT;
    qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    qos.representation().m_value.clear();
    qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    DataReader* reader = subscriber->create_datareader(topic, qos);
    if (reader == nullptr) {
        std::fprintf(stderr, "create_datareader failed\n");
        return 1;
    }
    std::printf("{\"event\":\"ready\"}\n");
    std::fflush(stdout);
    DynamicData::_ref_type data = DynamicDataFactory::get_instance()->create_data(type);
    auto end = std::chrono::steady_clock::now() + std::chrono::seconds(seconds);
    while (std::chrono::steady_clock::now() < end) {
        SampleInfo info;
        if (reader->take_next_sample(&data, &info) == RETCODE_OK) {
            if (info.valid_data) {
                int32_t id = 0;
                double value = 0;
                std::string unit;
                data->get_int32_value(id, data->get_member_id_by_name("id"));
                data->get_float64_value(value, data->get_member_id_by_name("value"));
                data->get_string_value(unit, data->get_member_id_by_name("unit"));
                std::printf("{\"id\":%d,\"value\":%.17g,\"unit\":\"%s\"}\n", id, value, unit.c_str());
                std::fflush(stdout);
            }
        } else {
            std::this_thread::sleep_for(std::chrono::milliseconds(20));
        }
    }
    return 0;
}

static int pub(DomainParticipant* participant, Topic* topic, DynamicType::_ref_type type, int id,
        double value, const char* unit)
{
    Publisher* publisher = participant->create_publisher(PUBLISHER_QOS_DEFAULT);
    DataWriterQos qos = DATAWRITER_QOS_DEFAULT;
    qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    qos.representation().m_value.clear();
    qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    DataWriter* writer = publisher->create_datawriter(topic, qos);
    if (writer == nullptr) {
        std::fprintf(stderr, "create_datawriter failed\n");
        return 1;
    }
    PublicationMatchedStatus matched;
    for (int i = 0; i < 1000; i++) {
        writer->get_publication_matched_status(matched);
        if (matched.current_count > 0) {
            break;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    if (matched.current_count == 0) {
        std::fprintf(stderr, "no reader matched\n");
        return 1;
    }
    DynamicData::_ref_type data = DynamicDataFactory::get_instance()->create_data(type);
    data->set_int32_value(data->get_member_id_by_name("id"), id);
    data->set_float64_value(data->get_member_id_by_name("value"), value);
    data->set_string_value(data->get_member_id_by_name("unit"), unit);
    if (writer->write(&data) != RETCODE_OK) {
        std::fprintf(stderr, "write failed\n");
        return 1;
    }
    std::printf("{\"event\":\"written\"}\n");
    std::fflush(stdout);
    std::this_thread::sleep_for(std::chrono::seconds(2));
    return 0;
}

int main(int argc, char** argv)
{
    if (argc < 3) {
        std::fprintf(stderr, "usage: %s sub DOMAIN SECONDS | pub DOMAIN ID VALUE UNIT\n", argv[0]);
        return 2;
    }
    DomainParticipant* participant = DomainParticipantFactory::get_instance()->create_participant(
        std::atoi(argv[2]), PARTICIPANT_QOS_DEFAULT);
    if (participant == nullptr) {
        std::fprintf(stderr, "create_participant failed\n");
        return 1;
    }
    DynamicType::_ref_type type = reading_type();
    TypeSupport support(new DynamicPubSubType(type));
    support.register_type(participant);
    Topic* topic = participant->create_topic("SensorReading", support.get_type_name(), TOPIC_QOS_DEFAULT);
    int rc;
    if (std::strcmp(argv[1], "sub") == 0 && argc == 4) {
        rc = sub(participant, topic, type, std::atoi(argv[3]));
    } else if (std::strcmp(argv[1], "pub") == 0 && argc == 6) {
        rc = pub(participant, topic, type, std::atoi(argv[3]), std::atof(argv[4]), argv[5]);
    } else {
        rc = 2;
    }
    participant->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(participant);
    return rc;
}
