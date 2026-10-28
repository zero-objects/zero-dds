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
if 'CombSample' not in s:
    s += '''
// The shape of tests/interop-corpus/combined-everything, cut down to
// the key path: a @key member of a keyless aggregate that holds a keyed
// aggregate and a sequence. Settles both the nesting rule and whether a
// sequence of non-primitives carries a DHEADER inside the holder.
@final struct CombLeaf {
  @key long k;
  short v;
};

@appendable struct CombMiddle {
  CombLeaf leaf;
  sequence<string> names;
};

@mutable struct CombSample {
  @key CombMiddle middle;
  long tail;
};
'''
    idl.write_text(s)

p = base / 'cyclone' / 'probe.c'
s = p.read_text()
if 'CombSample' not in s:
    s = s.replace(
        "  ChainOuter chain = {",
        'static char *comb_names[2] = { "", "\\xc3\\xbcber \\xe1\\x88\\xb4" };\n'
        "  CombSample comb;\n"
        "  memset (&comb, 0, sizeof comb);\n"
        "  comb.middle.leaf.k = -1234567;\n"
        "  comb.middle.leaf.v = -2;\n"
        "  comb.middle.names._length = 2;\n"
        "  comb.middle.names._maximum = 2;\n"
        "  comb.middle.names._release = false;\n"
        "  comb.middle.names._buffer = comb_names;\n"
        "  comb.tail = -1;\n"
        "  ChainOuter chain = {")
    s = s.replace(
        '    probe ("ChainOuter{mid={inner={0d..,0e..},r=0f..},tail=11..}", &ChainOuter_desc, &chain, xcdrv);\n',
        '    probe ("ChainOuter{mid={inner={0d..,0e..},r=0f..},tail=11..}", &ChainOuter_desc, &chain, xcdrv);\n'
        '    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)\n'
        '      probe ("CombSample (combined-everything key path)", &CombSample_desc, &comb, xcdrv);\n')
    p.write_text(s)

p = base / 'fastdds' / 'probe.cpp'
s = p.read_text()
if 'CombSample' not in s:
    s = s.replace(
        '    return 0;\n}',
        '''    CombLeaf cbl; cbl.k(-1234567); cbl.v(-2);
    CombMiddle cbm; cbm.leaf(cbl);
    cbm.names({std::string(""), std::string("\\xc3\\xbcber \\xe1\\x88\\xb4")});
    CombSample cbs; cbs.middle(cbm); cbs.tail(-1);
    probe<CombSamplePubSubType>("CombSample (combined-everything key path)", cbs);
    return 0;
}''')
    s = s.replace('#include <cstring>', '#include <cstring>\n#include <string>\n#include <vector>')
    p.write_text(s)

p = base / 'rti' / 'probe2.c'
s = p.read_text()
if 'CombSample' not in s:
    s = s.replace(
        '  PROBE (ChainOuter,',
        '''  PROBE (CombSample, do { sample.middle.leaf.k = -1234567; sample.middle.leaf.v = -2;
                          DDS_StringSeq_set_maximum (&sample.middle.names, 2);
                          DDS_StringSeq_set_length (&sample.middle.names, 2);
                          *DDS_StringSeq_get_reference (&sample.middle.names, 0) =
                              DDS_String_dup ("");
                          *DDS_StringSeq_get_reference (&sample.middle.names, 1) =
                              DDS_String_dup ("\\xc3\\xbcber \\xe1\\x88\\xb4");
                          sample.tail = -1; } while (0));
  PROBE (ChainOuter,''')
    p.write_text(s)
print('ok')
