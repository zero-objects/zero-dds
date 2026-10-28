#include <stdio.h>
#include <string.h>
#include "gen.h"
#include "genSupport.h"
#include "genPlugin.h"
int main(void) {
    for (unsigned r = 0; r < 3; r++) {
        char buf[12]; memset(buf, 0, sizeof buf);
        buf[0] = 0x00; buf[1] = 0x07; buf[2] = 0; buf[3] = 0;   /* CDR2_LE header */
        buf[4] = (char)r; buf[8] = 0x07;
        B out; B_initialize(&out);
        DDS_Boolean ok = BPlugin_deserialize_from_cdr_buffer(&out, buf, sizeof buf);
        printf("rti boolean=0x%02x  read=%s  flag=%u  n=%d\n", r,
               ok ? "accepted" : "REFUSED", (unsigned)out.flag, out.n);
    }
    printf("rti sizeof(B.flag)=%zu sizeof(C.letter)=%zu\n",
           sizeof(((B*)0)->flag), sizeof(((C*)0)->letter));
    return 0;
}
