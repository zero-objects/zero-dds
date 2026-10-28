// A MultiTopic (DDS 1.4 §2.2.2.3.4, Annex B.2) over generated types, through the DDS-PSM-Cxx
// classes: the topics `Order` and `Customer`, the MultiTopic `Joined` of the type `Sale` over
// `SELECT * FROM Order NATURAL JOIN Customer WHERE amount > %0`, and a DataReader<Sale> on it.
#include "multitopic_probe.hpp"
#include "dds/dds.hpp"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <thread>
#include <vector>

static int g_failures = 0;
#define EXPECT(cond, what)                                                     \
    do {                                                                       \
        if (!(cond)) {                                                         \
            std::printf("FAIL: %s\n", what);                                   \
            ++g_failures;                                                      \
        }                                                                      \
    } while (0)

int main(int argc, char** argv) {
    using namespace dds;
    const uint32_t domain_id = argc > 1 ? static_cast<uint32_t>(std::atoi(argv[1])) : 0;
    domain::DomainParticipant dp(domain_id);
    topic::Topic<mt::Order> orders(dp, "Order");
    topic::Topic<mt::Customer> customers(dp, "Customer");

    // The expression, the topics of its FROM clause and the result type must fit together.
    bool refused = false;
    try {
        const std::vector<std::string> none;
        topic::MultiTopic<mt::Sale> bad(dp, "Bad", "SELECT * FROM Order ORDER BY id", none.begin(),
                                        none.end());
    } catch (const core::Exception&) {
        refused = true;
    }
    EXPECT(refused, "an expression that is no TopicExpression is refused");

    const std::vector<std::string> params{"10"};
    topic::MultiTopic<mt::Sale> joined(
        dp, "Joined", "SELECT * FROM Order NATURAL JOIN Customer WHERE amount > %0", params.begin(),
        params.end());
    EXPECT(joined.expression() == "SELECT * FROM Order NATURAL JOIN Customer WHERE amount > %0",
           "the subscription expression");
    EXPECT(joined.expression_parameters().size() == 1 && joined.expression_parameters()[0] == "10",
           "the expression parameters");
    EXPECT(joined.name() == "Joined", "the name");
    const std::vector<std::string> lower{"1"};
    joined.expression_parameters(lower.begin(), lower.end());
    EXPECT(joined.expression_parameters()[0] == "1", "the parameters are replaced");
    const std::vector<std::string> higher{"10"};
    joined.expression_parameters(higher.begin(), higher.end());

    pub::Publisher publisher(dp);
    sub::Subscriber subscriber(dp);
    pub::DataWriter<mt::Order> order_writer(publisher, orders);
    pub::DataWriter<mt::Customer> customer_writer(publisher, customers);
    sub::DataReader<mt::Sale> reader(subscriber, joined);

    mt::Customer c1;
    c1.id = 1;
    c1.country = "DE";
    mt::Customer c2;
    c2.id = 2;
    c2.country = "FR";
    mt::Order o1;
    o1.id = 1;
    o1.amount = 5.0;
    mt::Order o2;
    o2.id = 2;
    o2.amount = 50.5;
    customer_writer.write(c1);
    customer_writer.write(c2);
    order_writer.write(o1);
    order_writer.write(o2);

    // Only the order above the parameter joins with its customer.
    std::vector<mt::Sale> got;
    const auto until = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    while (got.empty() && std::chrono::steady_clock::now() < until) {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (s.info().valid()) {
                got.push_back(s.data());
            }
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    EXPECT(got.size() == 1, "one joined sample");
    if (got.size() == 1) {
        EXPECT(got[0].id == 2 && got[0].country == "FR" && got[0].amount == 50.5,
               "the sample holds the members of both topics");
    }

    // Two more joined samples; the reader reads them by query, by instance and with a limit.
    mt::Customer c3;
    c3.id = 3;
    c3.country = "IT";
    mt::Customer c4;
    c4.id = 4;
    c4.country = "ES";
    mt::Order o3;
    o3.id = 3;
    o3.amount = 30.0;
    mt::Order o4;
    o4.id = 4;
    o4.amount = 40.0;
    customer_writer.write(c3);
    customer_writer.write(c4);
    order_writer.write(o3);
    order_writer.write(o4);
    const auto more = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    size_t held = 0;
    while (held < 2 && std::chrono::steady_clock::now() < more) {
        held = reader.read().length();
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    EXPECT(held == 2, "read leaves the joined samples in the reader");

    // A query over the members of the result type: only the order above 35.
    const std::vector<std::string> limit{"35"};
    sub::Query query(reader, "amount > %0", limit.begin(), limit.end());
    auto by_query = reader.select().content(query).read();
    EXPECT(by_query.length() == 1 && by_query.begin()->data().id == 4,
           "a read with a query returns the sample that meets it");

    // By instance: the handle of a sample reads that instance only.
    core::InstanceHandle third;
    for (const auto& s : reader.read()) {
        if (s.data().id == 3) {
            third = s.info().instance_handle();
        }
    }
    auto by_instance = reader.select().instance(third).read();
    EXPECT(by_instance.length() == 1 && by_instance.begin()->data().id == 3,
           "a read of an instance returns its sample");
    EXPECT(reader.select().max_samples(1).read().length() == 1, "max_samples limits the read");
    EXPECT(reader.select().instance(third).take().length() == 1, "take of an instance");
    EXPECT(reader.read().length() == 1, "and removes only that instance");

    // A reader on it keeps the MultiTopic: deleting the participant's MultiTopic is refused.
    bool kept = false;
    try {
        joined->close();
    } catch (const core::PreconditionNotMetError&) {
        kept = true;
    }
    EXPECT(kept, "close() of a MultiTopic with a reader is PRECONDITION_NOT_MET");

    if (g_failures == 0) {
        std::printf("GENERATED MULTITOPIC OK\n");
    }
    return g_failures;
}
