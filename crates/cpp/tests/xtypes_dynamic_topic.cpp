// Compile-only: Topic<DynamicData> with a DataWriter and DataReader, and a query on a field.
#include <dds/dds.hpp>
#include <dds/core/xtypes/DynamicData.hpp>

int main() {
    using namespace dds::core::xtypes;
    StructType type("m::Sensor", StructType(),
                    {MemberType("id", PrimitiveType<int32_t>(), annotation::Key()),
                     MemberType("value", PrimitiveType<double>())});
    dds::domain::DomainParticipant dp(0);
    dds::topic::Topic<DynamicData> topic(dp, "DynamicSensors", type);
    dds::pub::Publisher pub(dp);
    dds::sub::Subscriber sub(dp);
    dds::pub::DataWriter<DynamicData> writer(pub, topic);
    dds::sub::DataReader<DynamicData> reader(sub, topic);
    DynamicData sample(type);
    sample.value("id", int32_t(1));
    sample.value("value", 21.5);
    writer.write(sample);
    dds::sub::Query query(reader, "value > %0", std::vector<std::string>{"20"});
    for (auto& s : reader.select().content(query).take()) {
        if (s.info().valid()) {
            return s.data().value<double>("value") > 20.0 ? 0 : 1;
        }
    }
    return 0;
}
