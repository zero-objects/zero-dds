import subprocess, sys

def payloads(pcap):
    out = subprocess.run(["tshark","-r",pcap,"-Y","udp","-T","fields","-e","udp.payload"],
                         capture_output=True, text=True).stdout
    for line in out.splitlines():
        line = line.strip().replace(":","")
        if line:
            yield bytes.fromhex(line)

def user_data(pkt):
    if pkt[:4] != b"RTPS":
        return
    o = 20
    while o + 4 <= len(pkt):
        sid = pkt[o]; flags = pkt[o+1]
        end = "little" if flags & 1 else "big"
        nxt = int.from_bytes(pkt[o+2:o+4], end)
        body = o + 4
        size = nxt if nxt else len(pkt) - body
        if sid == 0x15:
            oti = int.from_bytes(pkt[body+2:body+4], end)
            writer = pkt[body+8:body+12]
            p2 = body + 4 + oti
            if flags & 0x02:                     # inline QoS: skip the ParameterList
                while p2 + 4 <= body + size:
                    pid = int.from_bytes(pkt[p2:p2+2], end)
                    plen = int.from_bytes(pkt[p2+2:p2+4], end)
                    p2 += 4
                    if pid == 1:
                        break
                    p2 += plen
            if writer[3] in (0x02, 0x03):
                yield pkt[p2: body + size]
        o = body + size

# string s; long b; octet c; double d; octet e; short f
def split(payload, ver):
    enc = payload[0:2]; opt = payload[2:4]
    d = payload[4:]
    da = 4 if ver == 2 else 8          # 8-byte primitive alignment
    o = 0
    slen = int.from_bytes(d[o:o+4], "little"); o += 4 + slen
    g1s = o; o += (-o) % 4; g1 = d[g1s:o]
    b = d[o:o+4]; o += 4
    c = d[o]; o += 1
    g2s = o; o += (-o) % da; g2 = d[g2s:o]
    dv = d[o:o+8]; o += 8
    e = d[o]; o += 1
    g3s = o; o += (-o) % 2; g3 = d[g3s:o]
    f = d[o:o+2]; o += 2
    return enc, opt, slen, g1s, g1, g2s, g2, g3s, g3, b, dv, c, e, f, len(d), o

for pcap, ver in ((sys.argv[1], int(sys.argv[2])),):
    print("==", pcap, "xcdr", ver)
    n = 0
    for pkt in payloads(pcap):
        for pl in user_data(pkt):
            enc, opt, slen, g1s, g1, g2s, g2, g3s, g3, b, dv, c, e, f, tot, used = split(pl, ver)
            n += 1
            print(f"  #{n:<2} enc={enc.hex()} opt={opt.hex()} slen={slen:>3} "
                  f"b={b.hex()} d={dv.hex()} c={c:02x} e={e:02x} f={f.hex()} "
                  f"|| fill[{g1s}]={g1.hex():<8} fill[{g2s}]={g2.hex():<16} fill[{g3s}]={g3.hex():<4} "
                  f"payload={tot} used={used}")
