# Single-use: this rewrites the probe sources in place and has
# already been applied. Kept for provenance — how `keyhash.idl` and
# the three probes grew to sixty-odd types — not to be run again.
# `base` used to be an absolute path on the test host; it is the
# harness directory beside this file now, so the script names no
# machine, but re-running it would apply its edit a second time.
import pathlib
HARNESS = pathlib.Path(__file__).resolve().parent.parent
bad = '"\\xc3\\xbcber \\xe1\\x88\\xb4"'
good = '"\\xc3\\xbc" "ber " "\\xe1\\x88\\xb4"'
for rel in ('cyclone/probe.c', 'fastdds/probe.cpp', 'rti/probe2.c'):
    p = pathlib.Path(HARNESS) / rel
    s = p.read_text()
    if bad in s:
        p.write_text(s.replace(bad, good))
        print('fixed', rel)
