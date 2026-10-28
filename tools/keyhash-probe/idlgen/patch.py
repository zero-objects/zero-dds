# Single-use: this rewrites the probe sources in place and has
# already been applied. Kept for provenance — how `keyhash.idl` and
# the three probes grew to sixty-odd types — not to be run again.
# `base` used to be an absolute path on the test host; it is the
# harness directory beside this file now, so the script names no
# machine, but re-running it would apply its edit a second time.
import re, sys, pathlib
HARNESS = pathlib.Path(__file__).resolve().parent.parent

base = pathlib.Path(HARNESS)

# --- Cyclone -------------------------------------------------------
p = base / 'cyclone' / 'probe.c'
s = p.read_text()
if 'OuterKeylessNested' not in s:
    s = s.replace(
        "  NoKeyMutable nkm = { .a = 0x12345678, .b = 20 };",
        "  NoKeyMutable nkm = { .a = 0x12345678, .b = 20 };\n"
        "  OuterKeylessNested okn = { .inner = { .a = 0x0a0a0a0a, .b = 0x0b0b0b0b }, .tail = 0x0c0c0c0c };\n"
        "  OuterKeyedNested okd = { .inner = { .a = 0x0a0a0a0a, .b = 0x0b0b0b0b }, .tail = 0x0c0c0c0c };\n"
        "  DeepOuter deep = { .mid = { .leaf = { .p = 0x0d0d0d0d, .q = 0x0e0e0e0e }, .r = 0x0f0f0f0f }, .tail = 0x11111111 };")
    s = s.replace(
        '''    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)
      probe ("NoKeyMutable{a=0x12345678,b=20}", &NoKeyMutable_desc, &nkm, xcdrv);''',
        '''    if (xcdrv == DDSI_RTPS_CDR_ENC_VERSION_2)
      probe ("NoKeyMutable{a=0x12345678,b=20}", &NoKeyMutable_desc, &nkm, xcdrv);
    probe ("OuterKeylessNested{inner={0a..,0b..},tail=0c..}", &OuterKeylessNested_desc, &okn, xcdrv);
    probe ("OuterKeyedNested{inner={0a..,0b..},tail=0c..}", &OuterKeyedNested_desc, &okd, xcdrv);
    probe ("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", &DeepOuter_desc, &deep, xcdrv);''', 1)
    p.write_text(s)
    print('cyclone patched')

# --- Fast DDS ------------------------------------------------------
p = base / 'fastdds' / 'probe.cpp'
s = p.read_text()
if 'OuterKeylessNested' not in s:
    s = s.replace(
        "    return 0;\n}",
        '''    NestedNoKey nnk; nnk.a(0x0a0a0a0a); nnk.b(0x0b0b0b0b);
    OuterKeylessNested okn; okn.inner(nnk); okn.tail(0x0c0c0c0c);

    NestedSomeKey nsk; nsk.a(0x0a0a0a0a); nsk.b(0x0b0b0b0b);
    OuterKeyedNested okd; okd.inner(nsk); okd.tail(0x0c0c0c0c);

    DeepLeaf dl; dl.p(0x0d0d0d0d); dl.q(0x0e0e0e0e);
    DeepMid dm; dm.leaf(dl); dm.r(0x0f0f0f0f);
    DeepOuter deep; deep.mid(dm); deep.tail(0x11111111);

    probe<OuterKeylessNestedPubSubType>("OuterKeylessNested{inner={0a..,0b..},tail=0c..}", okn);
    probe<OuterKeyedNestedPubSubType>("OuterKeyedNested{inner={0a..,0b..},tail=0c..}", okd);
    probe<DeepOuterPubSubType>("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", deep);
    return 0;
}''')
    p.write_text(s)
    print('fastdds patched')

# --- RTI -----------------------------------------------------------
p = base / 'rti' / 'probe2.c'
s = p.read_text()
if 'OuterKeylessNested' not in s:
    s = s.replace(
        "  PROBE (FooMut, do { sample.id = 0x12345678; sample.x = 10; sample.y = 20; } while (0));",
        '''  PROBE (FooMut, do { sample.id = 0x12345678; sample.x = 10; sample.y = 20; } while (0));
  PROBE (OuterKeylessNested, do { sample.inner.a = 0x0a0a0a0a; sample.inner.b = 0x0b0b0b0b;
                                  sample.tail = 0x0c0c0c0c; } while (0));
  PROBE (OuterKeyedNested, do { sample.inner.a = 0x0a0a0a0a; sample.inner.b = 0x0b0b0b0b;
                                sample.tail = 0x0c0c0c0c; } while (0));
  PROBE (DeepOuter, do { sample.mid.leaf.p = 0x0d0d0d0d; sample.mid.leaf.q = 0x0e0e0e0e;
                         sample.mid.r = 0x0f0f0f0f; sample.tail = 0x11111111; } while (0));''')
    p.write_text(s)
    print('rti patched')
