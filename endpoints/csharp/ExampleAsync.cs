// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (async): the same sensor-telemetry flow, but the subscriber does
// not own the run-loop. `AsyncReader.Stream()` is an IAsyncEnumerable; the
// consumer iterates it with `await foreach` — the idiomatic C# async model.
// Every field is decoded.

using System;
using System.Threading.Tasks;
using ZeroDDS;

namespace ZeroDDS.Examples
{
    public static class ExampleAsync
    {
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

        public static async Task<int> Main()
        {
            const int total = 5;
            var transport = new MemTransport();
            var client = new Client(transport);

            for (int i = 0; i < total; i++)
            {
                var rd = new Reading((uint)(0x2000 + i), 100.0f - i, $"sensor-{i:D2}");
                client.Write(rd.Marshal(Endianness.Little));
            }

            var reader = new AsyncReader(transport);
            int got = 0;
            await foreach (var body in reader.Stream())
            {
                var rd = Reading.Decode(body);
                Console.WriteLine($"async reading {got}: id=0x{rd.Id:x} value={rd.Value:F1} label=\"{rd.Label}\"");
                got++;
                if (got == total) break;
            }

            Console.WriteLine("ALL OK");
            return 0;
        }
    }
}
