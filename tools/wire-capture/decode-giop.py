import subprocess, sys

pcap = sys.argv[1]
out = subprocess.run(["tshark","-r",pcap,"-T","fields","-e","tcp.payload"],
                     capture_output=True, text=True).stdout
for line in out.splitlines():
    line = line.strip().replace(":","")
    if not line:
        continue
    p = bytes.fromhex(line)
    if p[:4] != b"GIOP" or p[7] != 0:
        continue
    end = "little" if (p[6] & 1) else "big"
    o = 12
    req = int.from_bytes(p[o:o+4], end); o += 4
    o += 4
    o += 2
    o += (-o) % 4
    klen = int.from_bytes(p[o:o+4], end); o += 4 + klen
    o += (-o) % 4
    olen = int.from_bytes(p[o:o+4], end); o += 4
    op = p[o:o+olen-1].decode("latin1"); o += olen
    o += (-o) % 4
    nsc = int.from_bytes(p[o:o+4], end); o += 4
    for _ in range(nsc):
        o += 4
        cl = int.from_bytes(p[o:o+4], end); o += 4 + cl
        o += (-o) % 4
    if op != "put":
        continue
    o += (-o) % 8
    base = o
    slen = int.from_bytes(p[o:o+4], end); o += 4 + slen
    g1s = o; o += (-o) % 4; g1 = p[g1s:o]
    b = p[o:o+4]; o += 4
    c = p[o]; o += 1
    g2s = o; o += (-o) % 8; g2 = p[g2s:o]
    dv = p[o:o+8]; o += 8
    e = p[o]; o += 1
    g3s = o; o += (-o) % 2; g3 = p[g3s:o]
    f = p[o:o+2]
    print(f"req {req:>3} {end:<6} slen={slen:>3} b={b.hex()} d={dv.hex()}"
          f" || fill[{g1s-base}..]={g1.hex():<8} fill[{g2s-base}..]={g2.hex():<16} fill[{g3s-base}..]={g3.hex()}")
