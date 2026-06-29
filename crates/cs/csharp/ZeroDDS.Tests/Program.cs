// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// C# smoke test for the ZeroDDS DDS-PSM-Cxx-1.0 API.
//
// Build + Run:
//   cargo build --release -p zerodds-c-api
//   bash crates/cs/csharp/build_cs_smoke.sh

using System;
using System.Threading;
using ZeroDDS;
using ZeroDDS.Cdr;
using ZeroDDS.Cond;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Pub;
using ZeroDDS.Sub;
using ZeroDDS.Topic;
using demo;
// Disambiguate the bare `DomainParticipantFactory` (used by the existing
// tests) to the static Domain class; the fluent singleton is referenced via
// the fully-qualified `ZeroDDS.DomainParticipantFactory.Instance`.
using DomainParticipantFactory = ZeroDDS.Domain.DomainParticipantFactory;

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
    // A writer and reader in the SAME participant on the same topic/type now
    // match via the intra-runtime route, so each side's matched TotalCount is 1.
    Expect(pmst.TotalCount == 1, "pmst one (same-participant match)");
    Expect(smst.TotalCount == 1, "smst one (same-participant match)");
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

// ---- Fluent factory + bytes shortcuts (website quickstart shape) ----
{
    var factory = ZeroDDS.DomainParticipantFactory.Instance;
    Expect(ReferenceEquals(factory, ZeroDDS.DomainParticipantFactory.Instance),
        "factory Instance is singleton");
    using var participant = factory.CreateParticipant(30);
    using var topic = participant.CreateBytesTopic("FluentChatter");
    using var publisher = participant.CreatePublisher();
    using var subscriber = participant.CreateSubscriber();
    using var writer = publisher.CreateBytesWriter(topic);
    using var reader = subscriber.CreateBytesReader(topic);

    writer.WaitForMatchedSubscription(1, TimeSpan.FromSeconds(5));
    reader.WaitForMatchedPublication(1, TimeSpan.FromSeconds(5));

    writer.Write("hello"u8.ToArray());
    bool got = reader.WaitForData(TimeSpan.FromSeconds(3));
    Expect(got, "fluent WaitForData saw data");

    int n = 0;
    string? text = null;
    foreach (var payload in reader.Take())
    {
        n++;
        text = System.Text.Encoding.UTF8.GetString(payload);
    }
    Expect(n == 1, $"fluent Take yielded 1 raw payload (got {n})");
    Expect(text == "hello", $"fluent payload = '{text}'");
}

// ---- TimeSpan → Duration bridge ----
{
    Expect(TimeSpan.FromSeconds(2).ToDuration() == Duration.FromSeconds(2),
        "TimeSpan 2s -> Duration");
    Expect(System.Threading.Timeout.InfiniteTimeSpan.ToDuration().IsInfinite,
        "InfiniteTimeSpan -> Duration.Infinite");
}

// ---- Typed topic/writer auto-resolving TypeSupport (website typed shape) ----
{
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(31);
    using var publisher = participant.CreatePublisher();
    using var subscriber = participant.CreateSubscriber();

    using var topic = participant.CreateTypedTopic<DemoTemp>("Temp");
    Expect(topic.TypeName == "demo::DemoTemp", $"typed type name = '{topic.TypeName}'");
    using var writer = publisher.CreateTypedWriter<DemoTemp>(topic);
    using var reader = subscriber.CreateTypedReader<DemoTemp>(topic);

    writer.WaitForMatched(1, Duration.FromSeconds(5));
    reader.WaitForMatched(1, Duration.FromSeconds(5));

    writer.Write(new DemoTemp { Celsius = 23, SensorId = "A7" });
    bool got = reader.WaitForData(TimeSpan.FromSeconds(3));
    Expect(got, "typed WaitForData saw data");
    var samples = reader.Take();
    Expect(samples.Count == 1 && samples[0].Data.Celsius == 23
        && samples[0].Data.SensorId == "A7", "typed roundtrip Celsius/SensorId");
}

// ---- async/await surface (website async shape) ----
{
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(32);
    using var topic = new Topic<byte[]>(participant, "AsyncT", new ByteSeqTraits());
    using var publisher = new Publisher(participant);
    using var subscriber = new Subscriber(participant);
    using var writer = new DataWriter<byte[]>(publisher, topic);
    using var reader = new DataReader<byte[]>(subscriber, topic);

    writer.WaitForMatched(1, Duration.FromSeconds(5));
    reader.WaitForMatched(1, Duration.FromSeconds(5));

    using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
    var ct = cts.Token;

    var payload = "ping"u8.ToArray();
    bool asyncOk = RunAsync().GetAwaiter().GetResult();
    Expect(asyncOk, "async WriteAsync/WaitForDataAsync/TakeAsync roundtrip");

    async System.Threading.Tasks.Task<bool> RunAsync()
    {
        await writer.WriteAsync(payload, ct);
        bool ready = await reader.WaitForDataAsync(TimeSpan.FromSeconds(3), ct);
        if (!ready) return false;
        int count = 0;
        await foreach (var sample in reader.TakeAsync(ct))
        {
            count++;
            if (sample.Data.Length != payload.Length) return false;
        }
        return count == 1;
    }

    // Cancellation surfaces as OperationCanceledException.
    using var cancelled = new CancellationTokenSource();
    cancelled.Cancel();
    bool threw = false;
    try { reader.WaitForDataAsync(TimeSpan.FromSeconds(1), cancelled.Token).GetAwaiter().GetResult(); }
    catch (OperationCanceledException) { threw = true; }
    Expect(threw, "WaitForDataAsync honours cancellation");
}

// ---- Partition QoS plumbed end-to-end (QB-cluster) ----
{
    // Mismatched partitions must NOT communicate; matching ones must.
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(40);
    using var topicMis = participant.CreateTypedTopic<DemoTemp>("PTopicMis");
    using var pubA = new Publisher(participant,
        new ZeroDDS.Qos.PublisherQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "A" } } });
    using var subB = new Subscriber(participant,
        new ZeroDDS.Qos.SubscriberQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "B" } } });
    using var wMis = new DataWriter<DemoTemp>(pubA, topicMis);
    using var rMis = new DataReader<DemoTemp>(subB, topicMis);
    bool matchedMismatch;
    try { wMis.WaitForMatched(1, Duration.FromMillis(800)); matchedMismatch = true; }
    catch (ZeroDDS.Core.TimeoutException) { matchedMismatch = false; }
    Expect(!matchedMismatch, "partition: mismatched A/B do NOT match");

    using var topicMatch = participant.CreateTypedTopic<DemoTemp>("PTopicMatch");
    using var pubX = new Publisher(participant,
        new ZeroDDS.Qos.PublisherQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "X" } } });
    using var subX = new Subscriber(participant,
        new ZeroDDS.Qos.SubscriberQos { Partition = new ZeroDDS.Qos.PartitionPolicy { Names = { "X" } } });
    using var wM = new DataWriter<DemoTemp>(pubX, topicMatch);
    using var rM = new DataReader<DemoTemp>(subX, topicMatch);
    bool matchedSame;
    try { wM.WaitForMatched(1, Duration.FromSeconds(3)); matchedSame = true; }
    catch (ZeroDDS.Core.TimeoutException) { matchedSame = false; }
    Expect(matchedSame, "partition: matching X/X DO match");
}

// ---- Keyed-instance lifecycle surface (QB-cluster) ----
{
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(41);
    using var topic = participant.CreateTypedTopic<KeyedRec>("LifecycleT");
    using var pub = new Publisher(participant);
    using var sub = new Subscriber(participant);
    using var w = new DataWriter<KeyedRec>(pub, topic);
    using var r = new DataReader<KeyedRec>(sub, topic);
    w.WaitForMatched(1, Duration.FromSeconds(5));
    r.WaitForMatched(1, Duration.FromSeconds(5));

    var sample = new KeyedRec { Key = 42, Payload = 1 };
    // register + lookup return a consistent, non-nil handle.
    var reg = w.RegisterInstance(sample);
    var look = w.LookupInstance(sample);
    Expect(!reg.IsNil && reg == look, "lifecycle: register/lookup_instance handle consistent");

    // dispose delivers NOT_ALIVE_DISPOSED (instance_state == 2) to the reader.
    w.Write(sample);
    r.WaitForData(TimeSpan.FromMilliseconds(500));
    r.Take();
    w.DisposeInstance(sample);
    bool sawDisposed = false;
    var sw = System.Diagnostics.Stopwatch.StartNew();
    while (!sawDisposed && sw.Elapsed < TimeSpan.FromSeconds(3))
    {
        if (!r.WaitForData(TimeSpan.FromMilliseconds(300))) continue;
        foreach (var s in r.Take()) if (s.Info.InstanceState == 2) sawDisposed = true;
    }
    Expect(sawDisposed, "lifecycle: dispose -> NOT_ALIVE_DISPOSED observed");

    // unregister round-trips without throwing.
    bool unregOk = true;
    try { w.UnregisterInstance(reg); } catch { unregOk = false; }
    Expect(unregOk, "lifecycle: unregister_instance round-trips");
}

// ---- ContentFilteredTopic filters samples (QB-cluster) ----
{
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(42);
    using var topic = participant.CreateTypedTopic<KeyedRec>("CftT");
    // KeyedRec is appendable: DHeader(int32) precedes Key(int32), Payload(int32).
    var schema = new System.Collections.Generic.List<ZeroDDS.Sub.CftField>
    {
        new("__dheader", ZeroDDS.Sub.CftFieldKind.Int32),
        new("Key", ZeroDDS.Sub.CftFieldKind.Int32),
        new("Payload", ZeroDDS.Sub.CftFieldKind.Int32),
    };
    using var cft = new ZeroDDS.Sub.ContentFilteredTopic<KeyedRec>(
        participant, "CftFilter", topic, "Payload > 4", null, schema);
    using var pub = new Publisher(participant);
    using var sub = new Subscriber(participant);
    using var w = new DataWriter<KeyedRec>(pub, topic);
    using var r = new DataReader<KeyedRec>(sub, cft);
    w.WaitForMatched(1, Duration.FromSeconds(5));
    r.WaitForMatched(1, Duration.FromSeconds(5));
    for (int i = 0; i < 10; i++) w.Write(new KeyedRec { Key = 1, Payload = i });
    var got = new System.Collections.Generic.List<int>();
    var sw = System.Diagnostics.Stopwatch.StartNew();
    while (sw.Elapsed < TimeSpan.FromSeconds(3))
    {
        if (!r.WaitForData(TimeSpan.FromMilliseconds(300))) { if (got.Count >= 5) break; continue; }
        foreach (var s in r.Take()) if (s.Info.ValidData) got.Add(s.Data.Payload);
    }
    got = new System.Collections.Generic.List<int>(new System.Collections.Generic.HashSet<int>(got));
    got.Sort();
    Expect(got.Count == 5 && got.TrueForAll(p => p > 4),
        $"cft: 'Payload > 4' delivered only matching [{string.Join(",", got)}]");
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

// Hand-written equivalent of `idlc csharp` output for
// `module demo { struct DemoTemp { long celsius; string sensor_id; }; }`.
// Mirrors the codegen convention so TypeSupportRegistry's reflection
// discovery resolves it from the bare CreateTypedTopic<DemoTemp> call.
namespace demo
{
    public sealed class DemoTemp
    {
        public int Celsius { get; init; }
        public string SensorId { get; init; } = "";
    }

    public sealed class DemoTempTypeSupport : ZeroDDS.Cdr.IDdsTopicType<DemoTemp>
    {
        public static readonly DemoTempTypeSupport Instance = new();

        public string TypeName => "demo::DemoTemp";
        public bool IsKeyed => false;
        public ZeroDDS.Cdr.ExtensibilityKind Extensibility =>
            ZeroDDS.Cdr.ExtensibilityKind.Appendable;

        public byte[] Encode(DemoTemp sample) =>
            Encode(sample, ZeroDDS.Cdr.EndianMode.LittleEndian);

        public byte[] Encode(DemoTemp sample, ZeroDDS.Cdr.EndianMode endian)
        {
            var w = new ZeroDDS.Cdr.Xcdr2Writer(endian);
            using (var __s = w.BeginAppendable())
            {
                w.WriteInt32(sample.Celsius);
                w.WriteString(sample.SensorId);
            }
            return w.ToArray();
        }

        public DemoTemp Decode(ReadOnlySpan<byte> bytes)
        {
            var r = new ZeroDDS.Cdr.Xcdr2Reader(bytes, ZeroDDS.Cdr.EndianMode.LittleEndian);
            var __s = r.BeginDHeader();
            int c = r.ReadInt32();
            string id = r.ReadString();
            r.EndDHeader(__s);
            return new DemoTemp { Celsius = c, SensorId = id };
        }

        public byte[] KeyHash(DemoTemp sample) => new byte[16];
    }

    // Keyed record mirroring `struct KeyedRec { @key long key; long payload; }`
    // (appendable, like the codegen output) for lifecycle + CFT tests.
    public sealed class KeyedRec
    {
        public int Key { get; init; }
        public int Payload { get; init; }
    }

    public sealed class KeyedRecTypeSupport : ZeroDDS.Cdr.IDdsTopicType<KeyedRec>
    {
        public static readonly KeyedRecTypeSupport Instance = new();

        public string TypeName => "demo::KeyedRec";
        public bool IsKeyed => true;
        public ZeroDDS.Cdr.ExtensibilityKind Extensibility =>
            ZeroDDS.Cdr.ExtensibilityKind.Appendable;

        public byte[] Encode(KeyedRec sample) =>
            Encode(sample, ZeroDDS.Cdr.EndianMode.LittleEndian);

        public byte[] Encode(KeyedRec sample, ZeroDDS.Cdr.EndianMode endian)
        {
            var w = new ZeroDDS.Cdr.Xcdr2Writer(endian);
            using (var __s = w.BeginAppendable())
            {
                w.WriteInt32(sample.Key);
                w.WriteInt32(sample.Payload);
            }
            return w.ToArray();
        }

        public KeyedRec Decode(ReadOnlySpan<byte> bytes)
        {
            var r = new ZeroDDS.Cdr.Xcdr2Reader(bytes, ZeroDDS.Cdr.EndianMode.LittleEndian);
            var __s = r.BeginDHeader();
            int k = r.ReadInt32();
            int p = r.ReadInt32();
            r.EndDHeader(__s);
            return new KeyedRec { Key = k, Payload = p };
        }

        public byte[] KeyHash(KeyedRec sample)
        {
            var kw = new ZeroDDS.Cdr.Xcdr2Writer(ZeroDDS.Cdr.EndianMode.BigEndian);
            kw.WriteInt32(sample.Key);
            var kb = kw.ToArray();
            var h = new byte[16];
            System.Array.Copy(kb, 0, h, 0, System.Math.Min(kb.Length, 16));
            return h;
        }
    }
}
