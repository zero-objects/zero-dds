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
if 'setvbuf' not in s:
    s = s.replace('int main()\n{', 'int main()\n{\n    setvbuf(stdout, nullptr, _IONBF, 0);')
    p.write_text(s)
    print('ok')
