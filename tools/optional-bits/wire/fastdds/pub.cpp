#include <cstdio>
#include <thread>
#include <chrono>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include "genPubSubTypes.hpp"
using namespace eprosima::fastdds::dds;
int main() {
    DomainParticipant *p = DomainParticipantFactory::get_instance()->create_participant(8, PARTICIPANT_QOS_DEFAULT);
    TypeSupport ts(new OnePubSubType());
    ts.register_type(p);
    Topic *t = p->create_topic("PadOne", ts.get_type_name(), TOPIC_QOS_DEFAULT);
    Publisher *pub = p->create_publisher(PUBLISHER_QOS_DEFAULT);
    DataWriterQos wq = DATAWRITER_QOS_DEFAULT; wq.data_sharing().off();
    DataWriter *w = pub->create_datawriter(t, wq);
    PublicationMatchedStatus st{};
    for (int i = 0; i < 200; i++) { w->get_publication_matched_status(st); if (st.current_count > 0) break; std::this_thread::sleep_for(std::chrono::milliseconds(100)); }
    printf("matched=%d\n", (int)st.current_count);
    for (int i = 0; i < 40; i++) {
        One s; s.a(0xab); w->write(&s);
        std::this_thread::sleep_for(std::chrono::milliseconds(200));
    }
    return 0;
}
