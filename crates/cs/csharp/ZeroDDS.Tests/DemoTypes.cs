// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

using System;
using ZeroDds.Wire;

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
