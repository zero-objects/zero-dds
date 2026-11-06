// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C# subscriber of tests/integration/tests/bindings_ffi.rs: the C#
// binding of crates/cs over libzerodds through DllImport
// (zerodds-ffi-loader-1.0 §3.3), with libzerodds on LD_LIBRARY_PATH.
//
//   dotnet FfiSub.dll <domain> <topic> <type name> <samples>
//
// The protocol of sub.c: `EVENT matched`, `EVENT sample <hex>` per body,
// `EVENT received n=<count>`, one more take after standard input closed,
// `EVENT done`.

using System;
using System.Diagnostics;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Qos;
using ZeroDDS.Sub;
using ZeroDDS.Topic;

namespace ZeroDdsFfiSub;

/// <summary>The body of a sample as the publisher's type name carries it, unread.</summary>
internal sealed class BodyTraits : ITopicTraits<byte[]>
{
    private readonly string _typeName;

    public BodyTraits(string typeName) { _typeName = typeName; }

    public string TypeName => _typeName;
    // `integration::FfiSample` has no key: the reader's endpoint is NO_KEY,
    // as the Rust writer's is.
    public bool IsKeyed => false;
    public byte[] Encode(byte[] value) => value;
    public byte[] Decode(ReadOnlySpan<byte> bytes) => bytes.ToArray();
}

internal static class Program
{
    private static int TakeAll(DataReader<byte[]> reader)
    {
        int taken = 0;
        foreach (var sample in reader.Take())
        {
            if (!sample.Info.ValidData) continue;
            Console.WriteLine("EVENT sample " + Convert.ToHexString(sample.Data).ToLowerInvariant());
            taken++;
        }
        return taken;
    }

    private static int Main(string[] args)
    {
        if (args.Length != 4)
        {
            Console.Error.WriteLine("usage: FfiSub <domain> <topic> <type name> <samples>");
            return 2;
        }
        uint domain = uint.Parse(args[0]);
        int wanted = int.Parse(args[3]);
        using var dp = DomainParticipantFactory.CreateParticipant(domain);
        using var topic = new Topic<byte[]>(dp, args[1], new BodyTraits(args[2]));
        using var sub = new Subscriber(dp);
        var qos = new DataReaderQos
        {
            Reliability = new ReliabilityPolicy(ReliabilityKind.Reliable),
            History = new HistoryPolicy(HistoryKind.KeepAll),
        };
        using var reader = new DataReader<byte[]>(sub, topic, qos);
        try
        {
            reader.WaitForMatched(1, Duration.FromSeconds(10));
        }
        catch (Exception)
        {
            Console.WriteLine("EVENT unmatched");
            return 1;
        }
        Console.WriteLine("EVENT matched");
        int taken = 0;
        var clock = Stopwatch.StartNew();
        while (taken < wanted && clock.ElapsedMilliseconds < 10000)
        {
            long left = Math.Max(1, 10000 - clock.ElapsedMilliseconds);
            if (reader.WaitForData(Duration.FromMillis(left)))
            {
                taken += TakeAll(reader);
            }
        }
        Console.WriteLine($"EVENT received n={taken}");
        Console.In.ReadToEnd();
        TakeAll(reader);
        Console.WriteLine("EVENT done");
        return 0;
    }
}
