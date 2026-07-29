#!/usr/bin/env python3
# Clear the PF_X bit from PT_GNU_STACK in ELF64 files (what `execstack -c` does),
# so libraries load on kernels that refuse an executable stack.
import sys, struct
PT_GNU_STACK = 0x6474e551
def patch(path):
    with open(path, 'r+b') as f:
        d = bytearray(f.read())
        if d[:4] != b'\x7fELF' or d[4] != 2:  # ELF64 only
            return False
        e_phoff = struct.unpack_from('<Q', d, 0x20)[0]
        e_phentsize = struct.unpack_from('<H', d, 0x36)[0]
        e_phnum = struct.unpack_from('<H', d, 0x38)[0]
        changed = False
        for i in range(e_phnum):
            off = e_phoff + i * e_phentsize
            p_type = struct.unpack_from('<I', d, off)[0]
            if p_type == PT_GNU_STACK:
                p_flags = struct.unpack_from('<I', d, off + 4)[0]
                if p_flags & 0x1:
                    struct.pack_into('<I', d, off + 4, p_flags & ~0x1)
                    changed = True
        if changed:
            f.seek(0); f.write(d)
        return changed
for p in sys.argv[1:]:
    try:
        print(("patched " if patch(p) else "ok      ") + p)
    except Exception as e:
        print("skip    " + p + ": " + str(e))
