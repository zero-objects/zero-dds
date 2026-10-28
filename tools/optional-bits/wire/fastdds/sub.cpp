#include <thread>
#include <chrono>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include "genPubSubTypes.hpp"
using namespace eprosima::fastdds::dds;
int main() {
    DomainParticipant *p = DomainParticipantFactory::get_instance()->create_participant(8, PARTICIPANT_QOS_DEFAULT);
    TypeSupport ts(new OnePubSubType());
    ts.register_type(p);
    Topic *t = p->create_topic("PadOne", ts.get_type_name(), TOPIC_QOS_DEFAULT);
    Subscriber *s = p->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    DataReaderQos rq = DATAREADER_QOS_DEFAULT; rq.data_sharing().off();
    s->create_datareader(t, rq);
    std::this_thread::sleep_for(std::chrono::seconds(10));
    return 0;
}
