/* QEMU `virt`: RAM from 0x4000_0000; an ELF given with -kernel is loaded
   where its segments say and started at its entry on core 0. */
ENTRY(_start)

SECTIONS
{
    . = 0x40080000;
    .text : { KEEP(*(.text.boot)) *(.text .text.*) }
    .rodata : ALIGN(8) { *(.rodata .rodata.*) }
    .data : ALIGN(8) { *(.data .data.*) }
    .bss (NOLOAD) : ALIGN(16) {
        __bss_start = .;
        *(.bss .bss.*) *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }
    . = ALIGN(4096);
    __page_table = .;
    . += 4096;
    . = ALIGN(16);
    . += 0x20000;
    __stack0_top = .;
    . += 0x20000;
    __stack1_top = .;
    /DISCARD/ : { *(.comment) *(.note .note.*) *(.eh_frame .eh_frame_hdr) }
}
