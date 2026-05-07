// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// C# Smoke-Test fuer ZeroDDS DDS-PSM-Cxx-1.0-API.
//
// Build + Run:
//   cargo build --release -p zerodds-c-api
//   bash crates/cs/csharp/build_cs_smoke.sh

using System;
using System.Threading;
using ZeroDDS.Cond;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Pub;
using ZeroDDS.Sub;
using ZeroDDS.Topic;

int failures = 0;
void Expect(bool cond, string msg)
{
    if (!cond) { Console.Error.WriteLine($"FAIL: {msg}"); failures++; }
}

// ---- Lifecycle ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(0);
    Expect(dp.DomainId == 0, "domain id roundtrip");
}

// ---- Topic ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(1);
    using var t = new Topic<byte[]>(dp, "ChatterTopic", new ByteSeqTraits());
    Expect(t.Name == "ChatterTopic", $"topic name = '{t.Name}'");
    Expect(t.TypeName == "DDS::Bytes", $"topic type = '{t.TypeName}'");
}

// ---- Writer + Reader Lifecycle ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(2);
    using var t = new Topic<byte[]>(dp, "T", new ByteSeqTraits());
    using var pub = new Publisher(dp);
    using var dw = new DataWriter<byte[]>(pub, t);
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);

    var msg = new byte[] { 1, 2, 3, 4 };
    try { dw.Write(msg); } catch (DdsException e) { /* may fail if no peer */ Console.WriteLine(e.Message); }
}

// ---- Take on empty ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(3);
    using var t = new Topic<byte[]>(dp, "E", new ByteSeqTraits());
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);
    var samples = dr.Take();
    Expect(samples.Count == 0, "empty take returns 0");
}

// ---- Status getters ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(4);
    using var t = new Topic<byte[]>(dp, "S", new ByteSeqTraits());
    using var pub = new Publisher(dp);
    using var dw = new DataWriter<byte[]>(pub, t);
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);

    var pmst = dw.GetPublicationMatchedStatus();
    var smst = dr.GetSubscriptionMatchedStatus();
    Expect(pmst.TotalCount == 0, "pmst zero");
    Expect(smst.TotalCount == 0, "smst zero");
}

// ---- ReadCondition ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(6);
    using var t = new Topic<byte[]>(dp, "RC", new ByteSeqTraits());
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);
    using var rc = new ZeroDDS.SubCond.ReadCondition<byte[]>(dr, 0xFF, 0xFF, 0xFF);
    bool t1 = rc.TriggerValue;
    bool t2 = rc.TriggerValue;
    Expect(t1 == t2, "read cond stable");
}

// ---- Listener attach/detach ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(7);
    using var t = new Topic<byte[]>(dp, "L", new ByteSeqTraits());
    using var pub = new Publisher(dp);
    using var dw = new DataWriter<byte[]>(pub, t);
    var listener = new TestWriterListener();
    using var anchor = ZeroDDS.Listener.DataWriterListenerBridge<byte[]>.Attach(dw, listener);
    Expect(listener.MatchedCount == 0, "listener not yet fired (no poll-call)");
}

// ---- Extended status getters ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(8);
    using var t = new Topic<byte[]>(dp, "SX", new ByteSeqTraits());
    using var pub = new Publisher(dp);
    using var dw = new DataWriter<byte[]>(pub, t);
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);

    var ll = dw.GetLivelinessLostStatus();
    var od = dw.GetOfferedDeadlineMissedStatus();
    var oi = dw.GetOfferedIncompatibleQosStatus();
    var lc = dr.GetLivelinessChangedStatus();
    var rd = dr.GetRequestedDeadlineMissedStatus();
    var ri = dr.GetRequestedIncompatibleQosStatus();
    var sr = dr.GetSampleRejectedStatus();
    Expect(ll.TotalCount == 0 && od.TotalCount == 0 && oi.TotalCount == 0,
        "writer status zero");
    Expect(lc.AliveCount == 0 && rd.TotalCount == 0 && ri.TotalCount == 0
        && sr.TotalCount == 0, "reader status zero");
}

// ---- QoS-Konstruktoren ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(20,
        new ZeroDDS.Qos.DomainParticipantQos
        {
            UserData = new ZeroDDS.Qos.UserDataPolicy { Value = new byte[] { 1, 2, 3 } },
        });
    var topicQos = new ZeroDDS.Qos.TopicQos
    {
        Reliability = new ZeroDDS.Qos.ReliabilityPolicy(
            ZeroDDS.Qos.ReliabilityKind.BestEffort, ZeroDDS.Core.Duration.FromMillis(50)),
        History = new ZeroDDS.Qos.HistoryPolicy(ZeroDDS.Qos.HistoryKind.KeepLast, 5),
    };
    using var t = new Topic<byte[]>(dp, "QT", new ByteSeqTraits(), topicQos);
    using var pub = new Publisher(dp, new ZeroDDS.Qos.PublisherQos());
    using var dw = new DataWriter<byte[]>(pub, t,
        new ZeroDDS.Qos.DataWriterQos
        {
            History = new ZeroDDS.Qos.HistoryPolicy(ZeroDDS.Qos.HistoryKind.KeepLast, 10),
        });
    using var sub = new Subscriber(dp, new ZeroDDS.Qos.SubscriberQos());
    using var dr = new DataReader<byte[]>(sub, t,
        new ZeroDDS.Qos.DataReaderQos
        {
            Reliability = new ZeroDDS.Qos.ReliabilityPolicy(
                ZeroDDS.Qos.ReliabilityKind.BestEffort, ZeroDDS.Core.Duration.FromMillis(100)),
        });
    Expect(dp.DomainId == 20, "qos-constr participant alive");
    Expect(t.Name == "QT", "qos-constr topic alive");
}

// ---- Conditions / WaitSet ----
{
    using var ws = new WaitSet();
    using var gc = new GuardCondition();
    Expect(!gc.TriggerValue, "guard initially false");
    gc.SetTriggerValue(true);
    Expect(gc.TriggerValue, "guard set true");
    ws.AttachCondition(gc);
    var active = ws.Wait(Duration.FromMillis(100));
    Expect(active.Count == 1, $"waitset active count = {active.Count}");
}

if (failures == 0)
{
    Console.WriteLine("OK — all C# DDS-PSM-Cxx smoke tests passed.");
}
else
{
    Console.Error.WriteLine($"FAIL — {failures} tests failed.");
}
Environment.Exit(failures);

// Helper class — must come AFTER top-level statements.
class TestWriterListener : ZeroDDS.Listener.IDataWriterListener<byte[]>
{
    public int MatchedCount { get; private set; }
    public void OnLivelinessLost(DataWriter<byte[]> dw, ZeroDDS.Status.LivelinessLostStatus s) { }
    public void OnOfferedDeadlineMissed(DataWriter<byte[]> dw, ZeroDDS.Status.OfferedDeadlineMissedStatus s) { }
    public void OnOfferedIncompatibleQos(DataWriter<byte[]> dw, ZeroDDS.Status.OfferedIncompatibleQosStatus s) { }
    public void OnPublicationMatched(DataWriter<byte[]> dw, ZeroDDS.Status.PublicationMatchedStatus s)
        { MatchedCount = s.CurrentCount; }
}
