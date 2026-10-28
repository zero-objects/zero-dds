# Single-use: this rewrites the probe sources in place and has
# already been applied. Kept for provenance — how `keyhash.idl` and
# the three probes grew to sixty-odd types — not to be run again.
# `base` used to be an absolute path on the test host; it is the
# harness directory beside this file now, so the script names no
# machine, but re-running it would apply its edit a second time.
import pathlib
HARNESS = pathlib.Path(__file__).resolve().parent.parent
p = pathlib.Path(str(HARNESS) + "/cyclone/probe.c")
s = p.read_text()
new = ('    probe ("OuterKeylessNested{inner={0a..,0b..},tail=0c..}", &OuterKeylessNested_desc, &okn, xcdrv);\n'
       '    probe ("OuterKeyedNested{inner={0a..,0b..},tail=0c..}", &OuterKeyedNested_desc, &okd, xcdrv);\n'
       '    probe ("DeepOuter{mid={leaf={0d..,0e..},r=0f..},tail=11..}", &DeepOuter_desc, &deep, xcdrv);\n')
s = s.replace(new, "")
anchor = '      printf ("--- NoKeyMutable (XCDR1) --- skipped: @mutable requires XCDR2 in Cyclone\\n\\n");\n'
assert anchor in s, "anchor missing"
s = s.replace(anchor, anchor + new)
p.write_text(s)
print("ok")
