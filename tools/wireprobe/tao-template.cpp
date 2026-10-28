/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The TAO probe's write direction. `wireprobe-tao` substitutes @TYPE@
 * with the C++ name, then compiles this against `tao_idl -as`'s output.
 *
 * ## Why there is no byte-order setting here
 *
 * `ACE_OutputCDR::write_4` swaps on write only when libACE was built
 * with `ACE_ENABLE_SWAP_ON_WRITE`, and the build on the test host was
 * not — measured 2026-08-10: a `TAO_OutputCDR` constructed with
 * `ACE_CDR::BYTE_ORDER_BIG_ENDIAN` reports `do_byte_swap() == 1` and
 * still writes `01000000` for a `long` of 1. TAO tags the GIOP message
 * with the order it wrote in rather than converting.
 *
 * So this template writes host order, which on the test host is little
 * endian, and `wireprobe-tao` refuses `--endian be` outright. Writing
 * these bytes under a big-endian label would record a divergence the run
 * never measured. The read direction has no such limit — input swapping
 * in ACE is unconditional — so TAO is a big-endian witness for reading.
 *
 * ## Why the buffer carries a marker
 *
 * TAO writes nothing at an alignment gap. `ACE_OutputCDR::adjust` in
 * `ace/CDR_Stream.inl` computes the offset from `current_alignment_`,
 * hands the caller `wr_ptr() + offset` and advances `wr_ptr` past both —
 * it copies no octet into the offset. So the fill this probe records is
 * whatever its buffer held, and with `TAO_OutputCDR((size_t)0, …)` that
 * was a page ACE had just taken from the kernel. Those zeros were read
 * as TAO's, and TAO's fill rows were counted byte-identical.
 *
 * They are not. Over GIOP/TCP (`tools/wire-capture`) TAO carries the
 * **previous request's** octets in the fill, exactly as omniORB does —
 * measured with a long and a short call alternating through one
 * connection, so the short call's fill spans land on the long call's
 * body. No buffer content is a function of the sample there, so no
 * buffer choice records TAO's wire, and `0xAA` marks the octets TAO does
 * not determine instead of hiding them under a zero.
 *
 * The buffer is this probe's own and not ACE's, because that is the only
 * way to decide its content: the user-buffer constructor takes it
 * without copying or clearing ("it will *not* remove @a data, since it
 * did not allocated it"). If a sample ever outgrew it ACE would chain a
 * block of its own behind it, whose fill would be unmarked; the loop
 * below already walks the chain, and the corpus's largest payload is
 * three orders of magnitude under the capacity.
 */
#include <cstdio>
#include <cstring>
#include <type_traits>

#include <tao/CDR.h>
#include <ace/Message_Block.h>

#include "genC.h"

/* `alignas` so ACE's `mb_align` finds the start already on
 * `ACE_CDR::MAX_ALIGNMENT` and leaves it where it is. */
alignas(ACE_CDR::MAX_ALIGNMENT) static char ZD_BUFFER[262144];

int main() {
    @TYPE@ sample;
#include "fill.inc"

    std::memset(ZD_BUFFER, 0xAA, sizeof(ZD_BUFFER));
    TAO_OutputCDR cdr(ZD_BUFFER, sizeof(ZD_BUFFER), (int)ACE_CDR::BYTE_ORDER_NATIVE);
    if (!(cdr << sample)) {
        std::fprintf(stderr, "tao refused to marshal the sample\n");
        return 1;
    }
    for (const ACE_Message_Block *block = cdr.begin(); block != 0; block = block->cont()) {
        std::fwrite(block->rd_ptr(), 1, block->length(), stdout);
    }
    return 0;
}
