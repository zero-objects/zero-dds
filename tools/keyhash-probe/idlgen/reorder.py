# Single-use: this rewrites the probe sources in place and has
# already been applied. Kept for provenance — how `keyhash.idl` and
# the three probes grew to sixty-odd types — not to be run again.
# `base` used to be an absolute path on the test host; it is the
# harness directory beside this file now, so the script names no
# machine, but re-running it would apply its edit a second time.
import pathlib
HARNESS = pathlib.Path(__file__).resolve().parent.parent
p = pathlib.Path(str(HARNESS) + '/fastdds/probe.cpp')
s = p.read_text()
line = '    probe<CombSamplePubSubType>("CombSample (combined-everything key path)", cbs);\n'
if line in s and not s.rstrip().endswith('}\n' + line.strip()):
    s = s.replace(line, '')
    s = s.replace('    return 0;\n}', line + '    return 0;\n}')
    p.write_text(s)
    print('moved')
