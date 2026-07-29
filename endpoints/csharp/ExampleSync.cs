// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
// five typed Reading(Id, Value, Label) samples and delivers them; the
// subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.

using System;
using ZeroDDS;

namespace ZeroDDS.Examples
{
    public static class ExampleSync
    {
        // Reading { uint32 Id; float Value; string Label }.
        private readonly struct Reading
        {
            public readonly uint Id;
            public readonly float Value;
            public readonly string Label;

            public Reading(uint id, float value, string label)
            {
                Id = id;
                Value = value;
                Label = label;
            }

            public byte[] Marshal(Endianness endian)
            {
                var w = new Writer(endian);
                w.PutU32(Id);
                w.PutF32(Value);
                w.PutString(Label);
                return w.Bytes();
            }

            public static Reading Decode(byte[] body)
            {
                var r = new Reader(body, Endianness.Little);
                return new Reading(r.GetU32(), r.GetF32(), r.GetString());
            }
        }

        public static int Main()
        {
            const int total = 5;
            var transport = new MemTransport();
            var client = new Client(transport);

            for (int i = 0; i < total; i++)
            {
                var rd = new Reading((uint)(0x1000 + i), 20.0f + i * 0.5f, $"bay-{i:D2}");
                client.Write(rd.Marshal(Endianness.Little));
            }

            int got = 0;
            while (got < total)
            {
                var body = client.Poll();
                if (body == null) break;
                var rd = Reading.Decode(body);
                Console.WriteLine($"sync reading {got}: id=0x{rd.Id:x} value={rd.Value:F1} label=\"{rd.Label}\"");
                got++;
            }

            if (got != total)
            {
                Console.Error.WriteLine($"incomplete: got {got} of {total}");
                return 1;
            }
            Console.WriteLine("ALL OK");
            return 0;
        }
    }
}
