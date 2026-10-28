// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// reliable_pair.hpp — the RELIABLE, KEEP_ALL writer and reader on one
// participant that the generated-code roundtrip tests write through and read
// back. Matched on return, so the first write reaches the reader.

#ifndef ZERODDS_CPP_TESTS_RELIABLE_PAIR_HPP
#define ZERODDS_CPP_TESTS_RELIABLE_PAIR_HPP

#include "dds/dds.hpp"

template <typename T>
struct ReliablePair {
    dds::pub::Publisher pub;
    dds::sub::Subscriber sub;
    dds::pub::DataWriter<T> writer;
    dds::sub::DataReader<T> reader;

    ReliablePair(const dds::domain::DomainParticipant& dp, const dds::topic::Topic<T>& topic)
        : pub(dp), sub(dp), writer(pub, topic, writer_qos()), reader(sub, topic, reader_qos()) {
        writer->wait_for_matched(1, dds::core::Duration(5, 0));
        reader->wait_for_matched(1, dds::core::Duration(5, 0));
    }

    static dds::pub::qos::DataWriterQos writer_qos() {
        dds::pub::qos::DataWriterQos wq;
        wq << dds::core::policy::Reliability::Reliable();
        wq << dds::core::policy::History::KeepAll();
        return wq;
    }

    static dds::sub::qos::DataReaderQos reader_qos() {
        dds::sub::qos::DataReaderQos rq;
        rq << dds::core::policy::Reliability::Reliable();
        rq << dds::core::policy::History::KeepAll();
        return rq;
    }
};

#endif // ZERODDS_CPP_TESTS_RELIABLE_PAIR_HPP
