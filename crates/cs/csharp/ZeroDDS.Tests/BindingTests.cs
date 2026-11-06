// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C# binding over the C API (libzerodds, DllImport): what the entities do
// through it, held to DDS 1.4, zerodds-ffi-loader-1.0 §3.3 and
// zerodds-listener-callbacks-1.1 §7.2. Every test runs in the DDS domain that
// tests/integration/with_domains.py leased for the run (ZERODDS_TEST_DOMAINS),
// each with topics of its own.
//
//   python3 tests/integration/with_domains.py 1 -- dotnet test crates/cs/csharp/ZeroDDS.Tests
//
// with libzerodds (`cargo build -p zerodds-c-api`) on LD_LIBRARY_PATH.

using System;
using System.Collections.Generic;
using System.Threading;
using Xunit;
using ZeroDDS;
using ZeroDDS.Cond;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Pub;
using ZeroDDS.Sub;
using ZeroDDS.Topic;
using demo;
using DomainParticipantFactory = ZeroDDS.Domain.DomainParticipantFactory;

namespace ZeroDdsBindingTests;

public sealed class BindingTests
{
    /// <summary>The leased domain of the run.</summary>
    private static uint Domain()
    {
        string? leased = Environment.GetEnvironmentVariable("ZERODDS_TEST_DOMAINS");
        if (string.IsNullOrEmpty(leased))
        {
            throw new InvalidOperationException(
                "ZERODDS_TEST_DOMAINS is not set: run under tests/integration/with_domains.py");
        }
        return uint.Parse(leased.Split(',')[0]);
    }

    [Fact]
    public void ffiloader_3_3_a_participant_a_topic_and_a_reader_are_created_through_dllimport()
    {
        using var dp = DomainParticipantFactory.CreateParticipant(Domain());
        Assert.Equal(Domain(), dp.DomainId);
        using var t = new Topic<byte[]>(dp, "BindingTopic", new ByteSeqTraits());
        Assert.Equal("BindingTopic", t.Name);
        // A byte[] topic has the built-in type DDS::Bytes of DDS-XTypes 1.3.
        Assert.Equal("DDS::Bytes", t.TypeName);
        using var sub = new Subscriber(dp);
        using var dr = new DataReader<byte[]>(sub, t);
        Assert.Empty(dr.Take());
    }

    [Fact]
    public void dcps_2_2_4_1_a_writer_and_a_reader_of_one_participant_match()
    {
        using var dp = DomainParticipantFactory.CreateParticipant(Domain());
        using var t = new Topic<byte[]>(dp, "BindingMatch", new ByteSeqTraits());
        using var pub = new Publisher(dp);
        using var dw = new DataWriter<byte[]>(pub, t);
        using var sub = new Subscriber(dp);
        using var dr = new DataReader<byte[]>(sub, t);
        dw.WaitForMatched(1, Duration.FromSeconds(5));
        dr.WaitForMatched(1, Duration.FromSeconds(5));
        Assert.Equal(1, dw.GetPublicationMatchedStatus().TotalCount);
        Assert.Equal(1, dr.GetSubscriptionMatchedStatus().TotalCount);
    }

    [Fact]
    public void dcps_2_2_4_1_the_statuses_of_a_writer_and_a_reader_without_a_match_count_nothing()
    {
        using var dp = DomainParticipantFactory.CreateParticipant(Domain());
        // The reader gets a topic of its own: a matched live writer makes
        // LIVELINESS_CHANGED alive_count 1 (DDS 1.4 §2.2.4.1).
        using var t = new Topic<byte[]>(dp, "BindingStatusWriter", new ByteSeqTraits());
        using var tr = new Topic<byte[]>(dp, "BindingStatusReader", new ByteSeqTraits());
        using var pub = new Publisher(dp);
        using var dw = new DataWriter<byte[]>(pub, t);
        using var sub = new Subscriber(dp);
        using var dr = new DataReader<byte[]>(sub, tr);
        Assert.Equal(0, dw.GetLivelinessLostStatus().TotalCount);
        Assert.Equal(0, dw.GetOfferedDeadlineMissedStatus().TotalCount);
        Assert.Equal(0, dw.GetOfferedIncompatibleQosStatus().TotalCount);
        Assert.Equal(0, dr.GetLivelinessChangedStatus().AliveCount);
        Assert.Equal(0, dr.GetRequestedDeadlineMissedStatus().TotalCount);
        Assert.Equal(0, dr.GetRequestedIncompatibleQosStatus().TotalCount);
        Assert.Equal(0, dr.GetSampleRejectedStatus().TotalCount);
    }

    [Fact]
    public void listenercallbacks_7_2_the_csharp_listener_bridge_fires_once_per_change_and_not_after_dispose()
    {
        using var dp = DomainParticipantFactory.CreateParticipant(Domain());
        using var t = new Topic<byte[]>(dp, "BindingListener", new ByteSeqTraits());
        using var pub = new Publisher(dp);
        using var dw = new DataWriter<byte[]>(pub, t);
        using var sub = new Subscriber(dp);
        using var dr = new DataReader<byte[]>(sub, t);

        var wl = new TestWriterListener();
        var rl = new TestReaderListener();
        var wreg = ZeroDDS.Listener.DataWriterListenerBridge<byte[]>.Attach(dw, wl);
        var rreg = ZeroDDS.Listener.DataReaderListenerBridge<byte[]>.Attach(dr, rl);

        // §4.1: no callback out of the set_listener call itself.
        Assert.Equal(0, wl.PublicationMatchedFired);
        Assert.Equal(0, rl.SubscriptionMatchedFired);

        // §6.1 / §6.2: the writer and the reader of one participant match; the poll sees
        // the counter delta, and the status value reaches the callback.
        dw.WaitForMatched(1, Duration.FromSeconds(5));
        dr.WaitForMatched(1, Duration.FromSeconds(5));
        ZeroDDS.Listener.ListenerPoll.PollAll();
        Assert.Equal(1, wl.PublicationMatchedFired);
        Assert.Equal(1, rl.SubscriptionMatchedFired);
        Assert.Equal(1, wl.MatchedCount);
        Assert.Equal(1, rl.MatchedCount);

        // §6: exactly once across polls: no delta, no callback.
        ZeroDDS.Listener.ListenerPoll.PollAll();
        Assert.Equal(1, wl.PublicationMatchedFired);

        // §6.3: data-available on a delivered sample.
        dw.Write(new byte[] { 7, 7, 7 });
        Assert.True(dr.WaitForData(TimeSpan.FromSeconds(5)));
        ZeroDDS.Listener.ListenerPoll.PollAll();
        Assert.True(rl.DataAvailableFired > 0);

        // §1.3 / §9: Dispose clears the slot before it frees the vtable; a later poll
        // finds no entry.
        wreg.Dispose();
        rreg.Dispose();
        int afterDetach = wl.PublicationMatchedFired + rl.SubscriptionMatchedFired + rl.DataAvailableFired;
        dw.Write(new byte[] { 8 });
        Assert.True(dr.WaitForData(TimeSpan.FromSeconds(5)));
        ZeroDDS.Listener.ListenerPoll.PollAll();
        Assert.Equal(afterDetach, wl.PublicationMatchedFired + rl.SubscriptionMatchedFired + rl.DataAvailableFired);
    }

    [Fact]
    public void dcps_2_2_2_1_a_triggered_guard_condition_ends_the_wait_of_its_wait_set()
    {
        using var ws = new WaitSet();
        using var gc = new GuardCondition();
        Assert.False(gc.TriggerValue);
        gc.SetTriggerValue(true);
        Assert.True(gc.TriggerValue);
        ws.AttachCondition(gc);
        Assert.Single(ws.Wait(Duration.FromSeconds(5)));
    }

    [Fact]
    public void ffiloader_3_3_a_byte_sample_is_written_and_taken_through_the_c_api()
    {
        var factory = ZeroDDS.DomainParticipantFactory.Instance;
        Assert.Same(factory, ZeroDDS.DomainParticipantFactory.Instance);
        using var participant = factory.CreateParticipant(Domain());
        using var topic = participant.CreateBytesTopic("BindingBytes");
        using var publisher = participant.CreatePublisher();
        using var subscriber = participant.CreateSubscriber();
        using var writer = publisher.CreateBytesWriter(topic);
        using var reader = subscriber.CreateBytesReader(topic);
        writer.WaitForMatchedSubscription(1, TimeSpan.FromSeconds(5));
        reader.WaitForMatchedPublication(1, TimeSpan.FromSeconds(5));

        writer.Write("hello"u8.ToArray());
        Assert.True(reader.WaitForData(TimeSpan.FromSeconds(5)));
        var texts = new List<string>();
        foreach (var payload in reader.Take())
        {
            texts.Add(System.Text.Encoding.UTF8.GetString(payload));
        }
        Assert.Equal(new[] { "hello" }, texts);
    }

    [Fact]
    public void ffiloader_3_3_a_typed_sample_is_written_and_taken_with_its_type_support()
    {
        using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(Domain());
        using var publisher = participant.CreatePublisher();
        using var subscriber = participant.CreateSubscriber();
        using var topic = participant.CreateTypedTopic<DemoTemp>("BindingTemp");
        Assert.Equal("demo::DemoTemp", topic.TypeName);
        using var writer = publisher.CreateTypedWriter<DemoTemp>(topic);
        using var reader = subscriber.CreateTypedReader<DemoTemp>(topic);
        writer.WaitForMatched(1, Duration.FromSeconds(5));
        reader.WaitForMatched(1, Duration.FromSeconds(5));

        writer.Write(new DemoTemp { Celsius = 23, SensorId = "A7" });
        Assert.True(reader.WaitForData(TimeSpan.FromSeconds(5)));
        var samples = reader.Take();
        Assert.Single(samples);
        Assert.Equal(23, samples[0].Data.Celsius);
        Assert.Equal("A7", samples[0].Data.SensorId);
    }

    [Fact]
    public void dcps_2_2_3_13_a_writer_and_a_reader_of_different_partitions_do_not_match()
    {
        using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(Domain());
        using var topicMis = participant.CreateTypedTopic<DemoTemp>("BindingPartitionApart");
        using var pubA = new Publisher(participant,
            new ZeroDDS.Qos.PublisherQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "A" } } });
        using var subB = new Subscriber(participant,
            new ZeroDDS.Qos.SubscriberQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "B" } } });
        using var wMis = new DataWriter<DemoTemp>(pubA, topicMis);
        using var rMis = new DataReader<DemoTemp>(subB, topicMis);

        // A pair of one partition, created after the other pair: once it matched, the
        // participant has decided the pair before it.
        using var topicMatch = participant.CreateTypedTopic<DemoTemp>("BindingPartitionTogether");
        using var pubX = new Publisher(participant,
            new ZeroDDS.Qos.PublisherQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "X" } } });
        using var subX = new Subscriber(participant,
            new ZeroDDS.Qos.SubscriberQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "X" } } });
        using var wM = new DataWriter<DemoTemp>(pubX, topicMatch);
        using var rM = new DataReader<DemoTemp>(subX, topicMatch);
        wM.WaitForMatched(1, Duration.FromSeconds(5));
        rM.WaitForMatched(1, Duration.FromSeconds(5));

        Assert.Equal(0, wMis.GetPublicationMatchedStatus().CurrentCount);
        Assert.Equal(0, rMis.GetSubscriptionMatchedStatus().CurrentCount);
    }

    [Fact]
    public void dcps_2_2_2_4_2_register_and_lookup_instance_agree_and_dispose_reaches_the_reader()
    {
        using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(Domain());
        using var topic = participant.CreateTypedTopic<KeyedRec>("BindingLifecycle");
        using var pub = new Publisher(participant);
        using var sub = new Subscriber(participant);
        using var w = new DataWriter<KeyedRec>(pub, topic);
        using var r = new DataReader<KeyedRec>(sub, topic);
        w.WaitForMatched(1, Duration.FromSeconds(5));
        r.WaitForMatched(1, Duration.FromSeconds(5));

        var sample = new KeyedRec { Key = 42, Payload = 1 };
        var reg = w.RegisterInstance(sample);
        Assert.False(reg.IsNil);
        Assert.Equal(reg, w.LookupInstance(sample));

        w.Write(sample);
        Assert.True(r.WaitForData(TimeSpan.FromSeconds(5)));
        r.Take();
        w.DisposeInstance(sample);
        // dispose reaches the reader as NOT_ALIVE_DISPOSED (instance_state 2)
        bool sawDisposed = false;
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(10);
        while (!sawDisposed && DateTime.UtcNow < deadline)
        {
            if (!r.WaitForData(TimeSpan.FromSeconds(1))) continue;
            foreach (var s in r.Take()) if (s.Info.InstanceState == 2) sawDisposed = true;
        }
        Assert.True(sawDisposed);
        w.UnregisterInstance(reg);
    }

    [Fact]
    public void dcps_2_2_2_3_3_a_content_filtered_topic_delivers_only_the_matching_samples()
    {
        using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(Domain());
        using var topic = participant.CreateTypedTopic<KeyedRec>("BindingFiltered");
        // KeyedRec is appendable: the DHEADER (int32) precedes Key (int32) and Payload (int32).
        var schema = new List<ZeroDDS.Sub.CftField>
        {
            new("__dheader", ZeroDDS.Sub.CftFieldKind.Int32),
            new("Key", ZeroDDS.Sub.CftFieldKind.Int32),
            new("Payload", ZeroDDS.Sub.CftFieldKind.Int32),
        };
        using var cft = new ZeroDDS.Sub.ContentFilteredTopic<KeyedRec>(
            participant, "BindingFilter", topic, "Payload > 4", null, schema);
        using var pub = new Publisher(participant);
        using var sub = new Subscriber(participant);
        using var w = new DataWriter<KeyedRec>(pub, topic);
        using var r = new DataReader<KeyedRec>(sub, cft);
        w.WaitForMatched(1, Duration.FromSeconds(5));
        r.WaitForMatched(1, Duration.FromSeconds(5));
        for (int i = 0; i < 10; i++) w.Write(new KeyedRec { Key = 1, Payload = i });
        // The last sample written passes the filter: once it arrived, so did the others.
        var got = new SortedSet<int>();
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(10);
        while (!got.Contains(9) && DateTime.UtcNow < deadline)
        {
            if (!r.WaitForData(TimeSpan.FromSeconds(1))) continue;
            foreach (var s in r.Take()) if (s.Info.ValidData) got.Add(s.Data.Payload);
        }
        Assert.Equal(new[] { 5, 6, 7, 8, 9 }, got);
    }
}

sealed class TestWriterListener : ZeroDDS.Listener.IDataWriterListener<byte[]>
{
    public int MatchedCount { get; private set; }
    public int PublicationMatchedFired { get; private set; }
    public void OnLivelinessLost(DataWriter<byte[]> dw, ZeroDDS.Status.LivelinessLostStatus s) { }
    public void OnOfferedDeadlineMissed(DataWriter<byte[]> dw, ZeroDDS.Status.OfferedDeadlineMissedStatus s) { }
    public void OnOfferedIncompatibleQos(DataWriter<byte[]> dw, ZeroDDS.Status.OfferedIncompatibleQosStatus s) { }
    public void OnPublicationMatched(DataWriter<byte[]> dw, ZeroDDS.Status.PublicationMatchedStatus s)
        { MatchedCount = s.CurrentCount; PublicationMatchedFired++; }
}

sealed class TestReaderListener : ZeroDDS.Listener.IDataReaderListener<byte[]>
{
    public int MatchedCount { get; private set; }
    public int SubscriptionMatchedFired { get; private set; }
    public int DataAvailableFired { get; private set; }
    public void OnDataAvailable(DataReader<byte[]> dr) { DataAvailableFired++; }
    public void OnSampleRejected(DataReader<byte[]> dr, ZeroDDS.Status.SampleRejectedStatus s) { }
    public void OnLivelinessChanged(DataReader<byte[]> dr, ZeroDDS.Status.LivelinessChangedStatus s) { }
    public void OnRequestedDeadlineMissed(DataReader<byte[]> dr, ZeroDDS.Status.RequestedDeadlineMissedStatus s) { }
    public void OnRequestedIncompatibleQos(DataReader<byte[]> dr, ZeroDDS.Status.RequestedIncompatibleQosStatus s) { }
    public void OnSubscriptionMatched(DataReader<byte[]> dr, ZeroDDS.Status.SubscriptionMatchedStatus s)
        { MatchedCount = s.CurrentCount; SubscriptionMatchedFired++; }
    public void OnSampleLost(DataReader<byte[]> dr, ZeroDDS.Status.SampleLostStatus s) { }
}
