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
if 'catch (const std::exception' not in s:
    s = s.replace('#include <cstring>', '#include <cstring>\n#include <exception>')
    old = '''    InstanceHandle_t h;
    memset(h.value, 0xAA, 16);
    bool ok = pst.compute_key(static_cast<const void*>(&sample), h, false);
    show("compute_key(data, h, force_md5=false)", ok, h);

    memset(h.value, 0xAA, 16);
    ok = pst.compute_key(static_cast<const void*>(&sample), h, true);
    show("compute_key(data, h, force_md5=true) ", ok, h);
    printf("\\n");'''
    new = '''    InstanceHandle_t h;
    try {
        memset(h.value, 0xAA, 16);
        bool ok = pst.compute_key(static_cast<const void*>(&sample), h, false);
        show("compute_key(data, h, force_md5=false)", ok, h);

        memset(h.value, 0xAA, 16);
        ok = pst.compute_key(static_cast<const void*>(&sample), h, true);
        show("compute_key(data, h, force_md5=true) ", ok, h);
    } catch (const std::exception& e) {
        printf("  compute_key threw %s\\n", e.what());
    }
    printf("\\n");'''
    assert old in s
    s = s.replace(old, new)
    p.write_text(s)
    print('ok')
