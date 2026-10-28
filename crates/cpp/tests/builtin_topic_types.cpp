// Compile and link only: the built-in topic types as topic types.
#include <dds/dds.hpp>
#include <dds/core/BuiltinTopicTypes.hpp>

int main() {
    dds::domain::DomainParticipant dp(0);
    dds::topic::Topic<dds::core::KeyedStringTopicType> topic(dp, "Keyed");
    dds::pub::Publisher pub(dp);
    dds::sub::Subscriber sub(dp);
    dds::pub::DataWriter<dds::core::KeyedStringTopicType> writer(pub, topic);
    dds::sub::DataReader<dds::core::KeyedStringTopicType> reader(sub, topic);
    writer.write(dds::core::KeyedStringTopicType("k", "v"));
    for (auto& s : reader.take()) {
        if (s.info().valid()) {
            return s.data().value() == "v" ? 0 : 1;
        }
    }
    dds::topic::Topic<dds::core::BytesTopicType> bytes(dp, "Bytes");
    dds::topic::Topic<dds::core::StringTopicType> text(dp, "Text");
    dds::topic::Topic<dds::core::KeyedBytesTopicType> keyed_bytes(dp, "KeyedBytes");
    return 0;
}
