# Single-use: this rewrites the probe sources in place and has
# already been applied. Kept for provenance — how `keyhash.idl` and
# the three probes grew to sixty-odd types — not to be run again.
# `base` used to be an absolute path on the test host; it is the
# harness directory beside this file now, so the script names no
# machine, but re-running it would apply its edit a second time.
import pathlib
HARNESS = pathlib.Path(__file__).resolve().parent.parent
base = pathlib.Path(HARNESS)

idl = base / 'keyhash.idl'
s = idl.read_text()
if 'ChainOuter' not in s:
    s += '''
// Distinguishes "recursion stops one level down" from "recursion stops
// at the first keyless aggregate": here every level is keyed, so a
// depth-limited reading gives p+q and an unlimited one gives p only.
@final struct ChainLeaf {
  @key long p;
  long q;
};

@final struct ChainMid {
  @key ChainLeaf inner;
  long r;
};

@final struct ChainOuter {
  @key ChainMid mid;
  long tail;
};
'''
    idl.write_text(s)

p = base / 'cyclone' / 'probe.c'
s = p.read_text()
if 'ChainOuter' not in s:
    s = s.replace(
        "  DeepOuter deep = { .mid",
        "  ChainOuter chain = { .mid = { .inner = { .p = 0x0d0d0d0d, .q = 0x0e0e0e0e }, .r = 0x0f0f0f0f }, .tail = 0x11111111 };\n"
        "  DeepOuter deep = { .mid")
    s = s.replace(
        '    probe ("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", &DeepOuter_desc, &deep, xcdrv);\n',
        '    probe ("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", &DeepOuter_desc, &deep, xcdrv);\n'
        '    probe ("ChainOuter{mid={inner={0d..,0e..},r=0f..},tail=11..}", &ChainOuter_desc, &chain, xcdrv);\n')
    p.write_text(s)

p = base / 'fastdds' / 'probe.cpp'
s = p.read_text()
if 'ChainOuter' not in s:
    s = s.replace(
        '    return 0;\n}',
        '''    ChainLeaf cl; cl.p(0x0d0d0d0d); cl.q(0x0e0e0e0e);
    ChainMid cm; cm.inner(cl); cm.r(0x0f0f0f0f);
    ChainOuter chain; chain.mid(cm); chain.tail(0x11111111);
    probe<ChainOuterPubSubType>("ChainOuter{mid={inner={0d..,0e..},r=0f..},tail=11..}", chain);
    return 0;
}''')
    p.write_text(s)

p = base / 'rti' / 'probe2.c'
s = p.read_text()
if 'ChainOuter' not in s:
    s = s.replace(
        '  PROBE (DeepOuter,',
        '''  PROBE (ChainOuter, do { sample.mid.inner.p = 0x0d0d0d0d; sample.mid.inner.q = 0x0e0e0e0e;
                          sample.mid.r = 0x0f0f0f0f; sample.tail = 0x11111111; } while (0));
  PROBE (DeepOuter,''')
    p.write_text(s)
print('ok')
