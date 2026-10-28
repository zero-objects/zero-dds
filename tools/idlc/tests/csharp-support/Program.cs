// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
// A real second-assembly consumer of fresh CLI output. No generated code lives here.
using System;
using System.Linq;
using System.Security.Cryptography;
using ZeroDDS;
using ZeroDDS.Cdr;
using bridge;

static void Check(bool condition, string why)
{
    if (!condition) throw new Exception(why);
}

static void Refuses<TException>(Action operation, string why) where TException : Exception
{
    try { operation(); }
    catch (Exception error) when (error.GetType() == typeof(TException)) { return; }
    throw new Exception("unexpected acceptance: " + why);
}

var support = TypeSupportRegistry.Resolve<Point>();
var emptySupport = TypeSupportRegistry.Resolve<Empty>();
Check(emptySupport.Encode(new Empty()).Length == 0, "empty aggregate acquired invented bytes");
Check(emptySupport.Decode(Array.Empty<byte>()) is not null, "empty aggregate was not decoded");
Check(ReferenceEquals(support, PointTypeSupport.Instance), "registry did not find fresh support");
Check(support.TypeName == "bridge::Point", "wrong IDL name");
Check(support.TypeObject.Length > 0, "no TypeObject");
Check(support.IsKeyed, "key lost");
var point = new Point { Id = 42, Reading = 65535 };
var little = Convert.FromHexString("2A000000FFFF");
var big = Convert.FromHexString("0000002AFFFF");
Check(support.Encode(point).SequenceEqual(little), "LE bytes differ from independent literal");
Check(support.Encode(point, EndianMode.BigEndian).SequenceEqual(big), "BE bytes differ");
var decoded = support.Decode(big, EndianMode.BigEndian, 1);
Check(decoded.Id == 42 && decoded.Reading == 65535, "BE value differs");
var key = Convert.FromHexString("0000002A000000000000000000000000");
Check(key.Length == 16, "key oracle must be exactly 16 bytes");
Check(support.KeyHash(point).SequenceEqual(key), "short holder must be zero-padded");
point.Reading = 7;
Check(support.KeyHash(point).SequenceEqual(key), "non-key field changed key identity");
var wrongKey = (byte[])key.Clone();
wrongKey[3] ^= 1;
Check(!support.KeyHash(point).SequenceEqual(wrongKey), "negative key oracle did not distinguish mutation");

var named = new Named { Name = "a", Value = 8 };
// A variable-length holder can still have a declared maximum below 16 bytes.
var shortName = new ShortName { Name = "abcdX", Value = 8 };
var shortKey = Convert.FromHexString("00000006616263645800000000000000");
Check(ShortNameTypeSupport.Instance.KeyHash(shortName).SequenceEqual(shortKey), "bounded string must be zero-padded");
shortName.Value = 9;
Check(ShortNameTypeSupport.Instance.KeyHash(shortName).SequenceEqual(shortKey), "bounded string non-key changed identity");
shortName.Name = "abcdY";
Check(ShortNameTypeSupport.Instance.KeyHash(shortName).SequenceEqual(
    Convert.FromHexString("00000006616263645900000000000000")), "bounded string key suffix lost");
// The declared maximum exceeds 16 even though this particular holder is short.
var expectedMd5 = MD5.HashData(Convert.FromHexString("000000026100"));
Check(NamedTypeSupport.Instance.KeyHash(named).SequenceEqual(expectedMd5), "maximum-size MD5 rule lost");
Refuses<ZeroDds.Wire.WireException.Truncated>(() => support.Decode(little.AsSpan(0, 5)), "truncated primitive");
Refuses<System.IO.InvalidDataException>(() => support.Decode(little.Concat(new byte[] { 0 }).ToArray()), "trailing octet");
Refuses<NotSupportedException>(() => support.Decode(little, EndianMode.LittleEndian, 0), "unsupported XCDR1");
Refuses<ArgumentOutOfRangeException>(() => support.Encode(point, (EndianMode)99), "invalid byte-order enum");

var extended = new Extended { Id = 13, Calibration = null };
var extendedSupport = ExtendedTypeSupport.Instance;
var absent = extendedSupport.Decode(extendedSupport.Encode(extended));
Check(absent.Id == 13 && absent.Calibration is null, "optional absence lost");
extended.Calibration = 1.25;
var present = extendedSupport.Decode(extendedSupport.Encode(extended));
Check(present.Calibration == 1.25, "optional presence lost");
Console.WriteLine("GENERATED_SUPPORT_PASS: registry, metadata presence, LE/BE, key oracles, optional, refusals");

if (args.Length == 0) return;
if (args.Length != 2 || args[0] != "--domain" || !uint.TryParse(args[1], out var domain))
    throw new ArgumentException("usage: Consumer [--domain INTEGER]");

using var participant = DomainParticipantFactory.Instance.CreateParticipant(domain);
var publisher = participant.CreatePublisher();
var subscriber = participant.CreateSubscriber();
using var topic = participant.CreateTypedTopic<Extended>("GeneratedSupportProbe");
using var reader = subscriber.CreateTypedReader<Extended>(topic);
using var writer = publisher.CreateTypedWriter<Extended>(topic);
writer.WaitForMatched(1, ZeroDDS.Core.Duration.FromSeconds(5));
reader.WaitForMatched(1, ZeroDDS.Core.Duration.FromSeconds(5));
writer.Write(extended);
Check(reader.WaitForData(TimeSpan.FromSeconds(5)), "no typed DCPS sample");
var received = reader.Take().Where(s => s.Info.ValidData).ToArray();
Check(received.Length == 1 && received[0].Data.Id == 13 && received[0].Data.Calibration == 1.25,
    "typed DCPS values differ");
var firstHandle = received[0].Info.InstanceHandle;
// No explicit registration: Write must establish the actual sample's instance.
var secondKey = new Extended { Id = 14, Calibration = 1.25 };
writer.Write(secondKey);
Check(reader.WaitForData(TimeSpan.FromSeconds(5)), "second key was not delivered");
var secondReceived = reader.Take().Where(s => s.Info.ValidData).ToArray();
Check(secondReceived.Length == 1 && secondReceived[0].Data.Id == 14 &&
    secondReceived[0].Data.Calibration == 1.25, "second key sample differs");
Check(!firstHandle.Equals(secondReceived[0].Info.InstanceHandle), "distinct keys collapsed into one instance");
extended.Calibration = 9.5;
writer.Write(extended);
Check(reader.WaitForData(TimeSpan.FromSeconds(5)), "same-key update was not delivered");
var updated = reader.Take().Where(s => s.Info.ValidData).ToArray();
Check(updated.Length == 1 && updated[0].Data.Id == 13 && updated[0].Data.Calibration == 9.5,
    "same-key changed data differs");
Check(firstHandle.Equals(updated[0].Info.InstanceHandle), "non-key data changed the reader instance");
Console.WriteLine("GENERATED_DCPS_PASS: two keys and stable reader instance across non-key data change");

using var emptyTopic = participant.CreateTypedTopic<Empty>("GeneratedEmptyProbe");
using var emptyReader = subscriber.CreateTypedReader<Empty>(emptyTopic);
using var emptyWriter = publisher.CreateTypedWriter<Empty>(emptyTopic);
emptyWriter.WaitForMatched(1, ZeroDDS.Core.Duration.FromSeconds(5));
emptyReader.WaitForMatched(1, ZeroDDS.Core.Duration.FromSeconds(5));
emptyWriter.Write(new Empty());
Check(emptyReader.WaitForData(TimeSpan.FromSeconds(5)), "no valid empty DDS sample");
var emptyReceived = emptyReader.Take().Where(s => s.Info.ValidData).ToArray();
Check(emptyReceived.Length == 1 && emptyReceived[0].Data is not null, "valid empty DDS sample became null");
Console.WriteLine("GENERATED_EMPTY_DCPS_PASS: zero-byte final aggregate decoded");
