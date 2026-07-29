# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Tests for the native Julia endpoint: byte-identity vs the Rust goldens, plus
# sync + async loopback. Run with `julia test.jl` (GOLDEN_DIR=build).

include("zerodds.jl")
using .ZeroDDS

golden_dir = get(ENV, "GOLDEN_DIR", "build")

function fixture(endian)
    w = ZeroDDS.Writer(endian)
    ZeroDDS.put_u32!(w, 0xA1B2C3D4)
    ZeroDDS.put_u16!(w, 0x1234)
    ZeroDDS.put_u8!(w, 0x5A)
    ZeroDDS.put_f32!(w, 3.5)
    ZeroDDS.put_u64!(w, 0x0102030405060708)
    ZeroDDS.put_string!(w, "bay-12")
    ZeroDDS.put_seq_u8!(w, UInt8[0xDE, 0xAD, 0xBE, 0xEF])
    ZeroDDS.bytes(w)
end

function sample_body(id)
    w = ZeroDDS.Writer(ZeroDDS.LE)
    ZeroDDS.put_u32!(w, id)
    ZeroDDS.put_u16!(w, 0)
    ZeroDDS.put_u8!(w, 0)
    ZeroDDS.bytes(w)
end

# --- byte identity ---
for (endian, file) in [(ZeroDDS.LE, "golden_le.bin"), (ZeroDDS.BE, "golden_be.bin")]
    got = fixture(endian)
    golden = read(joinpath(golden_dir, file))
    @assert got == golden "$file: not byte-identical (got $(length(got)) want $(length(golden)))"
    println("$file: $(length(golden)) bytes byte-identical")
end

# --- sync loopback ---
t = ZeroDDS.mem_transport()
c = ZeroDDS.Client(t)
for i in 0:4
    ZeroDDS.write!(c, sample_body(0x3000 + i))
end
for i in 0:4
    body = ZeroDDS.poll(c)
    @assert body !== nothing "sync: no sample $i"
    id = ZeroDDS.get_u32(ZeroDDS.Reader(body, ZeroDDS.LE))
    @assert id == 0x3000 + i "sync: id mismatch"
end
println("sync loopback: 5 samples OK")

# --- async loopback (Task + Channel) ---
t2 = ZeroDDS.mem_transport()
w = ZeroDDS.Client(t2)
for i in 0:4
    ZeroDDS.write!(w, sample_body(0x1000 + i))
end
r = ZeroDDS.start_reader(t2)
for i in 0:4
    body = ZeroDDS.recv(r)
    id = ZeroDDS.get_u32(ZeroDDS.Reader(body, ZeroDDS.LE))
    @assert id == 0x1000 + i "async: id mismatch"
end
ZeroDDS.stop!(r)
println("async loopback: 5 samples OK")
println("ALL OK")
