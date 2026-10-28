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
if 'WholeOuter' not in s:
    s += '''
// tests/interop-corpus/annotation-key-whole-struct, exactly.
@final struct WholeInner {
  long a;
  short b;
};

@final struct WholeOuter {
  @key WholeInner inner;
  long other;
};
'''
    idl.write_text(s)

p = base / 'cyclone' / 'probe.c'
s = p.read_text()
if 'WholeOuter' not in s:
    s = s.replace("  ChainOuter chain = {",
                  "  WholeOuter whole = { .inner = { .a = -1234567, .b = -2 }, .other = 1 };\n"
                  "  ChainOuter chain = {")
    s = s.replace('    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)\n      probe ("CombSample',
                  '    probe ("WholeOuter{inner={-1234567,-2},other=1}", &WholeOuter_desc, &whole, xcdrv);\n'
                  '    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)\n      probe ("CombSample')
    p.write_text(s)

p = base / 'fastdds' / 'probe.cpp'
s = p.read_text()
if 'WholeOuter' not in s:
    s = s.replace('    return 0;\n}',
                  '''    WholeInner wi; wi.a(-1234567); wi.b(-2);
    WholeOuter wo; wo.inner(wi); wo.other(1);
    probe<WholeOuterPubSubType>("WholeOuter{inner={-1234567,-2},other=1}", wo);
    return 0;
}''')
    p.write_text(s)

p = base / 'rti' / 'probe2.c'
s = p.read_text()
if 'WholeOuter' not in s:
    s = s.replace('  PROBE (ChainOuter,',
                  '  PROBE (WholeOuter, do { sample.inner.a = -1234567; sample.inner.b = -2;\n'
                  '                          sample.other = 1; } while (0));\n'
                  '  PROBE (ChainOuter,')
    p.write_text(s)
print('ok')
