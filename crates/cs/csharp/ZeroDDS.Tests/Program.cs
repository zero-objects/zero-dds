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
using ZeroDds.Wire;
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
    using var dp = DomainParticipantFactory.CreateParticipant(180);
    Expect(dp.DomainId == 180, "domain id roundtrip");
}

// ---- Topic ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(181);
    using var t = new Topic<byte[]>(dp, "ChatterTopic", new ByteSeqTraits());
    Expect(t.Name == "ChatterTopic", $"topic name = '{t.Name}'");
    Expect(t.TypeName == "DDS::Bytes", $"topic type = '{t.TypeName}'");
}

// ---- Writer + Reader Lifecycle ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(182);
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
    using var dp = DomainParticipantFactory.CreateParticipant(183);
    using var t = new Topic<byte[]>(dp, "E", new ByteSeqTraits());
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);
    var samples = dr.Take();
    Expect(samples.Count == 0, "empty take returns 0");
}

// ---- Status getters ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(184);
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
    using var dp = DomainParticipantFactory.CreateParticipant(186);
    using var t = new Topic<byte[]>(dp, "RC", new ByteSeqTraits());
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);
    using var rc = new ZeroDDS.SubCond.ReadCondition<byte[]>(dr, 0xFF, 0xFF, 0xFF);
    bool t1 = rc.TriggerValue;
    bool t2 = rc.TriggerValue;
    Expect(t1 == t2, "read cond stable");
}

// ---- Listener attach + active firing ----
// Vendor spec `zerodds-listener-callbacks-1.1` §4.1 (nothing is delivered
// synchronously out of set_listener), §6 (the poll fires on a counter delta,
// exactly once across polls) and §7.2 (the C# bridge).
{
    using var dp = DomainParticipantFactory.CreateParticipant(187);
    using var t = new Topic<byte[]>(dp, "L", new ByteSeqTraits());
    using var pub = new Publisher(dp);
    using var dw = new DataWriter<byte[]>(pub, t);
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, t);

    var wl = new TestWriterListener();
    var rl = new TestReaderListener();
    using var wreg = ZeroDDS.Listener.DataWriterListenerBridge<byte[]>.Attach(dw, wl);
    using var rreg = ZeroDDS.Listener.DataReaderListenerBridge<byte[]>.Attach(dr, rl);

    // §4.1 — no callback out of the set_listener call itself.
    Expect(wl.PublicationMatchedFired == 0, "no writer callback before the first poll");
    Expect(rl.SubscriptionMatchedFired == 0, "no reader callback before the first poll");

    // §6.1 / §6.2 — writer and reader in the same participant match over the
    // intra-runtime route, so the first poll sees a counter delta.
    int fired = ZeroDDS.Listener.ListenerPoll.PollAll();
    Expect(fired > 0, $"first poll fired callbacks (fired={fired})");
    Expect(wl.PublicationMatchedFired == 1,
        $"on_publication_matched fired once (n={wl.PublicationMatchedFired})");
    Expect(rl.SubscriptionMatchedFired == 1,
        $"on_subscription_matched fired once (n={rl.SubscriptionMatchedFired})");
    // The status value reaches the callback, not just the notification.
    Expect(wl.MatchedCount == 1, $"publication-matched status read back (CurrentCount={wl.MatchedCount})");
    Expect(rl.MatchedCount == 1, $"subscription-matched status read back (CurrentCount={rl.MatchedCount})");

    // §6 — exactly once across polls: no delta, no callback.
    int again = ZeroDDS.Listener.ListenerPoll.PollAll();
    Expect(wl.PublicationMatchedFired == 1,
        $"a poll without a delta fires nothing again (n={wl.PublicationMatchedFired}, fired={again})");

    // §6.3 — data-available on a delivered-sample delta.
    dw.Write(new byte[] { 7, 7, 7 });
    for (int i = 0; i < 100 && rl.DataAvailableFired == 0; i++)
    {
        ZeroDDS.Listener.ListenerPoll.PollAll();
        Thread.Sleep(10);
    }
    Expect(rl.DataAvailableFired > 0, $"on_data_available fired (n={rl.DataAvailableFired})");

    // §1.3 / §9 — Dispose clears the slot before it frees the vtable, so a
    // later poll finds no entry rather than a dangling pointer.
    wreg.Dispose();
    rreg.Dispose();
    int afterDetach = wl.PublicationMatchedFired + rl.SubscriptionMatchedFired;
    ZeroDDS.Listener.ListenerPoll.PollAll();
    Expect(wl.PublicationMatchedFired + rl.SubscriptionMatchedFired == afterDetach,
        "no callback after the registration was disposed");
}

// ---- Extended status getters ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(188);
    using var t = new Topic<byte[]>(dp, "SX", new ByteSeqTraits());
    // The reader gets a topic of its own: on "SX" it would match the writer,
    // and a matched live writer makes LIVELINESS_CHANGED alive_count 1
    // (DDS 1.4 §2.2.4.1) as soon as the match lands, which made this check
    // depend on how fast that is. Unmatched, every count below is 0.
    using var tr = new Topic<byte[]>(dp, "SX-reader", new ByteSeqTraits());
    using var pub = new Publisher(dp);
    using var dw = new DataWriter<byte[]>(pub, t);
    using var sub = new Subscriber(dp);
    using var dr = new DataReader<byte[]>(sub, tr);

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

// ---- QoS constructors ----
{
    using var dp = DomainParticipantFactory.CreateParticipant(200,
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
    Expect(dp.DomainId == 200, "qos-constr participant alive");
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
    using var participant = factory.CreateParticipant(210);
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
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(211);
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
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(212);
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
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(220);
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
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(221);
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
    using var participant = ZeroDDS.DomainParticipantFactory.Instance.CreateParticipant(222);
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
    public int PublicationMatchedFired { get; private set; }
    public void OnLivelinessLost(DataWriter<byte[]> dw, ZeroDDS.Status.LivelinessLostStatus s) { }
    public void OnOfferedDeadlineMissed(DataWriter<byte[]> dw, ZeroDDS.Status.OfferedDeadlineMissedStatus s) { }
    public void OnOfferedIncompatibleQos(DataWriter<byte[]> dw, ZeroDDS.Status.OfferedIncompatibleQosStatus s) { }
    public void OnPublicationMatched(DataWriter<byte[]> dw, ZeroDDS.Status.PublicationMatchedStatus s)
        { MatchedCount = s.CurrentCount; PublicationMatchedFired++; }
}

class TestReaderListener : ZeroDDS.Listener.IDataReaderListener<byte[]>
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
        public Omg.Types.ExtensibilityKind Extensibility =>
            Omg.Types.ExtensibilityKind.Appendable;

        private static Endianness Order(ZeroDDS.Cdr.EndianMode endian) => endian switch
        {
            ZeroDDS.Cdr.EndianMode.LittleEndian => Endianness.Little,
            ZeroDDS.Cdr.EndianMode.BigEndian => Endianness.Big,
            _ => throw new ArgumentOutOfRangeException(nameof(endian)),
        };

        public byte[] Encode(DemoTemp sample) =>
            Encode(sample, ZeroDDS.Cdr.EndianMode.LittleEndian);

        public byte[] Encode(DemoTemp sample, ZeroDDS.Cdr.EndianMode endian)
        {
            var w = new Writer(Order(endian), 4, PlCdr1Framing.Defaults(), false);
            var d = w.WriteDheader();
            w.WriteI32(sample.Celsius);
            w.WriteString(sample.SensorId);
            w.FinishDheader(d);
            return w.Finish();
        }

        public DemoTemp Decode(ReadOnlySpan<byte> bytes)
        {
            var r = new Reader(bytes.ToArray(), Endianness.Little, 4, PlCdr1Framing.Defaults());
            uint declared = r.ReadDheader();
            long end = r.ReaderPosition() + declared;
            int c = r.ReadI32();
            string id = r.ReadStringAlloc();
            r.ReaderSkipTo(end);
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
        public Omg.Types.ExtensibilityKind Extensibility =>
            Omg.Types.ExtensibilityKind.Appendable;

        private static Endianness Order(ZeroDDS.Cdr.EndianMode endian) => endian switch
        {
            ZeroDDS.Cdr.EndianMode.LittleEndian => Endianness.Little,
            ZeroDDS.Cdr.EndianMode.BigEndian => Endianness.Big,
            _ => throw new ArgumentOutOfRangeException(nameof(endian)),
        };

        public byte[] Encode(KeyedRec sample) =>
            Encode(sample, ZeroDDS.Cdr.EndianMode.LittleEndian);

        public byte[] Encode(KeyedRec sample, ZeroDDS.Cdr.EndianMode endian)
        {
            var w = new Writer(Order(endian), 4, PlCdr1Framing.Defaults(), false);
            var d = w.WriteDheader();
            w.WriteI32(sample.Key);
            w.WriteI32(sample.Payload);
            w.FinishDheader(d);
            return w.Finish();
        }

        public KeyedRec Decode(ReadOnlySpan<byte> bytes)
        {
            var r = new Reader(bytes.ToArray(), Endianness.Little, 4, PlCdr1Framing.Defaults());
            uint declared = r.ReadDheader();
            long end = r.ReaderPosition() + declared;
            int k = r.ReadI32();
            int p = r.ReadI32();
            r.ReaderSkipTo(end);
            return new KeyedRec { Key = k, Payload = p };
        }

        public byte[] KeyHash(KeyedRec sample)
        {
            var kw = new Writer(Endianness.Big, 4, PlCdr1Framing.Defaults(), false);
            kw.WriteI32(sample.Key);
            return kw.ZeroPadded();
        }
    }
}
